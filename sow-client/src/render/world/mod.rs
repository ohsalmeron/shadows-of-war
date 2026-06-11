pub mod buildings;
pub mod effects;
pub mod fleets;
pub mod movers;
pub mod nameplates;
pub mod projectiles;
pub mod railways;
pub mod utils;

use crate::app::SowApp;
use crate::config::ClientVisualConfig;
use crate::hud::nameplate::*;

#[derive(Copy, Clone)]
pub(crate) struct VisPlayer<'a> {
    pub player: &'a sow_core::protocol::PlayerSnapshot,
    pub center: egui::Pos2,
    pub pc: egui::Color32,
    pub lod_presence: f32,
    pub nameplate_size: f32,
}

pub(crate) struct RenderContext<'a> {
    pub painter: &'a egui::Painter,
    pub wall_secs: f64,
    pub current_tick: u64,
    pub dot_r: f32,
    pub visible_players: &'a [VisPlayer<'a>],
    pub zoom_scaled: f32,
    pub player_colors: &'a [egui::Color32],
    pub terrain: &'a [u8],
    pub sf: f32,
}

impl SowApp {
    pub(crate) fn render_world_overlays(&mut self, ctx: &egui::Context, sf: f32) {
        // Register world_buildings layer first to draw behind world_overlays
        let _ = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("world_buildings"),
        ));

        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("world_overlays"),
        ));

        // Register world_nameplates in Middle order so it is above ALL Background layers
        // (buildings, effects, projectiles) but below Foreground (GUI/HUD panels).
        let _ = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("world_nameplates"),
        ));
        let wall_secs = self.time.start_time.elapsed().as_secs_f64();
        let current_tick = self
            .sim
            .current_snapshot
            .as_ref()
            .map(|s| s.tick)
            .unwrap_or(0);

        // Configuration variables removed from GameConfig
        let visual_config = ClientVisualConfig::default();
        let dot_r = visual_config.ui_lod_dot_radius;

        let player_count = self
            .sim
            .current_snapshot
            .as_ref()
            .map_or(0, |s| s.players.len());
        let mut visible_players = Vec::with_capacity(player_count);
        let dt = self.ui.raw_input.predicted_dt;
        let smooth_factor = 1.0 - (-10.0 * dt).exp();
        if let Some(snap) = &self.sim.current_snapshot {
            for player in &snap.players {
                if player.tile_count == 0 || !player.alive {
                    continue;
                }

                let (avg_col, avg_row) = (player.nameplate_x, player.nameplate_y);

                let target_cx = avg_col + 0.5;
                let target_cy = avg_row + 0.5;

                // Early frustum cull on target position — skip interpolation for off-screen players
                let target_screen_x =
                    (target_cx * self.input.camera_zoom + self.input.camera_x) / sf;
                let target_screen_y =
                    (target_cy * self.input.camera_zoom + self.input.camera_y) / sf;
                if target_screen_x < -100.0
                    || target_screen_x > self.input.screen_w + 100.0
                    || target_screen_y < -100.0
                    || target_screen_y > self.input.screen_h + 100.0
                {
                    continue;
                }

                // Smooth position interpolation
                let pos = self
                    .ui
                    .label_positions
                    .entry(player.id)
                    .or_insert((target_cx, target_cy));
                let dx = target_cx - pos.0;
                let dy = target_cy - pos.1;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > 50.0 {
                    pos.0 = target_cx;
                    pos.1 = target_cy;
                } else if dist > 0.1 {
                    pos.0 += dx * smooth_factor;
                    pos.1 += dy * smooth_factor;
                    self.ui.egui_ctx.request_repaint();
                } else {
                    pos.0 = target_cx;
                    pos.1 = target_cy;
                }

                let screen_x = (pos.0 * self.input.camera_zoom + self.input.camera_x) / sf;
                let screen_y = (pos.1 * self.input.camera_zoom + self.input.camera_y) / sf;

                // Frustum cull
                if screen_x < -100.0
                    || screen_x > self.input.screen_w + 100.0
                    || screen_y < -100.0
                    || screen_y > self.input.screen_h + 100.0
                {
                    continue;
                }

                let center = egui::pos2(screen_x, screen_y);
                let pc = nameplate_matte_player_rgb(player.color);

                // `lod_presence` uses zoom (when zoomed out, dots only). `sizing_presence`
                // does not, so nameplate font sizes stay stable and egui's glyph atlas is not
                // invalidated every scroll step (fixes garbled glyphs). Font size is rounded
                // to whole points for fewer distinct `FontId`s.
                // Normalize tile count so text size is consistent regardless of total map size.
                // 40_000 is a reference 200x200 map.
                let map_area = (self.sim.map_w * self.sim.map_h).max(1) as f32;
                let normalized_tiles = player.tile_count as f32 * (40_000.0 / map_area);
                let importance = (normalized_tiles * 0.35).max(0.15);

                let lod_presence = importance * (self.input.camera_zoom / sf);

                let target_size = if player.nameplate_size > 0.1 {
                    player.nameplate_size
                } else {
                    0.0
                };

                let size_entry = self.ui.label_sizes.entry(player.id).or_insert(target_size);

                let ds = target_size - *size_entry;
                if ds.abs() > 0.01 {
                    let rate = if ds > 0.0 {
                        visual_config.nameplate_size_grow_rate
                    } else {
                        10.0
                    };
                    let smooth_factor_size = 1.0 - (-rate * dt).exp();
                    *size_entry += ds * smooth_factor_size;
                    self.ui.egui_ctx.request_repaint();
                } else {
                    *size_entry = target_size;
                }

                let nameplate_size = *size_entry;

                visible_players.push(VisPlayer {
                    player,
                    center,
                    pc,
                    lod_presence,
                    nameplate_size,
                });
            }

            // Sort once: local player last (drawn on top), then humans, nations, presence (desc)
            let my_id = self.sim.my_player_id.unwrap_or(0);
            visible_players.sort_unstable_by(|a, b| {
                let a_is_me = a.player.id == my_id;
                let b_is_me = b.player.id == my_id;
                if a_is_me != b_is_me {
                    return a_is_me.cmp(&b_is_me);
                }
                let a_is_human = a.player.player_type == sow_core::player::PlayerType::Human;
                let b_is_human = b.player.player_type == sow_core::player::PlayerType::Human;
                if a_is_human != b_is_human {
                    return b_is_human.cmp(&a_is_human);
                }
                let a_is_nation = a.player.id < 200;
                let b_is_nation = b.player.id < 200;
                if a_is_nation != b_is_nation {
                    return b_is_nation.cmp(&a_is_nation);
                }
                b.lod_presence
                    .partial_cmp(&a.lod_presence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let zoom_scaled = self.input.camera_zoom / sf;

            // Clean up expired upgrade animations once per frame
            let now = web_time::Instant::now();
            self.ui
                .active_upgrades
                .retain(|anim| now.duration_since(anim.start_time) < anim.duration);

            // Rebuild player colors only when the player list size changes
            let player_count = snap.players.len();
            if self.ui.cached_player_count != player_count {
                let max_pid = snap.players.iter().map(|p| p.id).max().unwrap_or(0) as usize;
                self.ui
                    .cached_player_colors
                    .resize(max_pid + 1, egui::Color32::GRAY);
                self.ui.cached_player_count = player_count;
            }
            for p in &snap.players {
                let id = p.id as usize;
                let rgb = p.color;
                if id < self.ui.cached_player_colors.len() {
                    self.ui.cached_player_colors[id] = egui::Color32::from_rgb(
                        (rgb[0] * 255.0) as u8,
                        (rgb[1] * 255.0) as u8,
                        (rgb[2] * 255.0) as u8,
                    );
                }
            }

            // Take the player colors out temporarily to break the shared borrow without allocating
            let player_colors = std::mem::take(&mut self.ui.cached_player_colors);

            let terrain = self
                .gfx
                .map_renderer
                .as_ref()
                .map(|mr| mr.terrain.as_slice())
                .unwrap_or(&[]);

            let ctx_struct = RenderContext {
                painter: &painter,
                wall_secs,
                current_tick,
                dot_r,
                visible_players: &visible_players,
                zoom_scaled,
                player_colors: &player_colors,
                terrain,
                sf,
            };

            fleets::render(
                &mut self.ui,
                &self.sim,
                &self.input,
                &self.time,
                &self.gfx,
                &ctx_struct,
            );
            railways::render(
                &mut self.ui,
                &self.sim,
                &self.input,
                &self.time,
                &self.gfx,
                &ctx_struct,
            );
            buildings::render(
                &mut self.ui,
                &self.sim,
                &self.input,
                &self.time,
                &self.gfx,
                &ctx_struct,
            );
            effects::render(
                &mut self.ui,
                &self.sim,
                &self.input,
                &self.time,
                &self.gfx,
                &ctx_struct,
            );
            projectiles::render(
                &mut self.ui,
                &self.sim,
                &self.input,
                &self.time,
                &self.gfx,
                &ctx_struct,
            );
            nameplates::render(
                &mut self.ui,
                &self.sim,
                &self.input,
                &self.time,
                &self.gfx,
                &ctx_struct,
            );

            nameplates::render_death_nameplates(&mut self.ui, &self.input, sf, now);

            let middle_painter = painter.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("floating_notices"),
            ));
            let visual_config = ClientVisualConfig::default();

            // Render floating notices (Gold rewards) on top
            self.ui.floating_notices.retain(|notice| {
                painter.ctx().request_repaint();
                let elapsed = now.duration_since(notice.start_time).as_secs_f32();
                let duration = notice.duration.as_secs_f32();
                if elapsed >= duration {
                    return false;
                }

                let t = elapsed / duration;
                let current_wy = notice.world_y - t * 6.5; // rise by 6.5 units
                let screen_x = (self.input.camera_x + notice.world_x * self.input.camera_zoom) / sf;
                let screen_y = (self.input.camera_y + current_wy * self.input.camera_zoom) / sf;

                if screen_x >= -150.0
                    && screen_x <= self.input.screen_w + 150.0
                    && screen_y >= -150.0
                    && screen_y <= self.input.screen_h + 150.0
                {
                    let pos = egui::pos2(screen_x, screen_y);
                    let alpha = ((1.0 - t) * 255.0) as u8;
                    let text_color = egui::Color32::from_rgba_unmultiplied(
                        notice.color.r(),
                        notice.color.g(),
                        notice.color.b(),
                        alpha,
                    );
                    let bounce_scale = if elapsed < 0.5 {
                        let anim_t = elapsed / 0.5;
                        nameplates::spring_overshoot(anim_t)
                    } else if elapsed > duration - 0.5 {
                        let anim_t = (duration - elapsed) / 0.5;
                        nameplates::spring_overshoot(anim_t).clamp(0.0, 1.2)
                    } else {
                        1.0
                    };
                    // Quantize to whole pixels for egui glyph atlas cache reuse
                    let font_size = (visual_config.gold_reward_notice_font_size * bounce_scale)
                        .round()
                        .max(1.0);

                    let font_id = egui::FontId::proportional(font_size);
                    sow_ui::widgets::paint_emoji_text_at(
                        &middle_painter,
                        pos,
                        egui::Align2::CENTER_CENTER,
                        &notice.text,
                        font_id,
                        text_color,
                        true,
                    );
                }
                true
            });

            // Restore the player colors back to UI state to preserve the pre-allocated capacity
            self.ui.cached_player_colors = player_colors;
        }
    }
}
