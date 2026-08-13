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

/// Below this `zoom_scaled` value, buildings and railways hard-return — no collection,
/// no draws, no rail graph rebuild. Single source of truth for the world-layer cull floor.
pub(crate) const BUILDINGS_HIDE_FLOOR: f32 = 1.0;

pub(crate) struct RenderContext<'a> {
    pub painter: &'a egui::Painter,
    pub wall_secs: f64,
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
        let my_id = self.sim.my_player_id.unwrap_or(0);
        if let Some(snap) = &self.sim.current_snapshot {
            for player in &snap.players {
                if player.tile_count == 0 || !player.alive {
                    continue;
                }

                let dev = sow_ui_kit::theme::dev_config::DevConfig::get();
                // During the deploy (Spawning) phase, fog_explored is empty by
                // design (visibility.rs sets visible=ALL, explored=NONE so fog
                // starts fresh post-deploy). Gating names on explored here made
                // every other player's nameplate vanish during deploy. Skip the
                // fog gate while spawning; names render for everyone who spawned.
                let is_spawning =
                    matches!(snap.phase, sow_core::game::GamePhase::Spawning { .. });
                if dev.fog_of_war && player.id != my_id && !is_spawning {
                    let col = player.centroid_x.floor() as i32;
                    let row = player.centroid_y.floor() as i32;
                    if col >= 0
                        && row >= 0
                        && col < self.sim.map_w as i32
                        && row < self.sim.map_h as i32
                    {
                        let t_idx = (row * self.sim.map_w as i32 + col) as u32;
                        if !self.sim.fog_explored.contains(t_idx) {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }

                let target_cx = player.centroid_x + 0.5;
                let target_cy = player.centroid_y + 0.5;

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
                } else {
                    let screen_dist = dist * self.input.camera_zoom / sf;
                    if screen_dist > 0.5 && dist > 0.05 {
                        pos.0 += dx * smooth_factor;
                        pos.1 += dy * smooth_factor;
                        self.ui.egui_ctx.request_repaint();
                    } else {
                        pos.0 = target_cx;
                        pos.1 = target_cy;
                    }
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
                let pc = nameplate_matte_player_rgb(
                    player
                        .team
                        .map_or(player.color, sow_core::player::team_territory_rgb),
                );

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

                // ~ territory side length in world units. The old 24.0 cap saturated at just
                // 576 tiles, erasing size differences between every real territory.
                let target_size = (player.tile_count as f32).sqrt().clamp(0.2, 150.0);

                let size_entry = self.ui.label_sizes.entry(player.id).or_insert(target_size);

                let ds = target_size - *size_entry;
                if ds.abs() > 0.2 {
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

            // Sort once: other humans on top of all (drawn last), then local player, then nations, then others.
            let my_id = self.sim.my_player_id.unwrap_or(0);
            visible_players.sort_unstable_by(|a, b| {
                let a_prec = if a.player.player_type == sow_core::player::PlayerType::Human {
                    if a.player.id == my_id { 1 } else { 2 }
                } else {
                    0
                };
                let b_prec = if b.player.player_type == sow_core::player::PlayerType::Human {
                    if b.player.id == my_id { 1 } else { 2 }
                } else {
                    0
                };
                if a_prec != b_prec {
                    return a_prec.cmp(&b_prec);
                }
                let a_is_nation = a.player.id < 200;
                let b_is_nation = b.player.id < 200;
                if a_is_nation != b_is_nation {
                    return a_is_nation.cmp(&b_is_nation); // true (nation) comes after false (tribe), so they draw on top of tribes
                }
                b.lod_presence
                    .partial_cmp(&a.lod_presence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let zoom_scaled = self.input.camera_zoom / sf;
            let now = web_time::Instant::now();

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
                let rgb = p
                    .team
                    .map_or(p.color, sow_core::player::team_territory_rgb);
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

            let terrain: &[u8] = unsafe {
                let mr_ptr = self
                    .gfx
                    .map_renderer
                    .as_ref()
                    .map(|mr| mr.terrain.as_slice() as *const [u8])
                    .unwrap_or(&[] as *const [u8]);
                &*mr_ptr
            };

            let ctx_struct = RenderContext {
                painter: &painter,
                wall_secs,
                dot_r,
                visible_players: &visible_players,
                zoom_scaled,
                player_colors: &player_colors,
                terrain,
                sf,
            };

            fleets::render(
                &self.sim,
                &self.input,
                &self.time,
                &ctx_struct,
            );
            railways::render(
                &mut self.ui,
                &self.sim,
                &self.input,
                &self.time,
                &mut self.gfx,
                &ctx_struct,
            );
            buildings::render(
                &mut self.ui,
                &self.sim,
                &self.input,
                &self.time,
                &mut self.gfx,
                &ctx_struct,
            );
            effects::render(
                &mut self.ui,
                &self.input,
                &ctx_struct,
            );
            projectiles::render(
                &mut self.ui,
                &self.sim,
                &self.input,
                &self.time,
                &mut self.gfx,
                &ctx_struct,
            );
            nameplates::render(
                &mut self.ui,
                &self.sim,
                &self.input,
                &mut self.gfx,
                &ctx_struct,
            );

            let middle_painter = painter.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("floating_notices"),
            ));
            let visual_config = ClientVisualConfig::default();

            // Render floating notices (Gold rewards) on top
            if !self.ui.floating_notices.is_empty() {
                painter.ctx().request_repaint();
            }
            self.ui.floating_notices.retain(|notice| {
                let elapsed = now.duration_since(notice.start_time).as_secs_f32();
                let duration = notice.duration.as_secs_f32();
                if elapsed >= duration {
                    return false;
                }

                let t = elapsed / duration;
                let current_wy = notice.world_y - t * 6.5; // rise by 6.5 units
                let screen_x = (self.input.camera_x + notice.world_x * self.input.camera_zoom) / sf;
                let screen_y = (self.input.camera_y + current_wy * self.input.camera_zoom) / sf;

                if sow_ui_kit::theme::dev_config::DevConfig::get().vfx_floating_notices
                    && screen_x >= -150.0
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
                        sow_ui::ui::animation::spring_overshoot(anim_t)
                    } else if elapsed > duration - 0.5 {
                        let anim_t = (duration - elapsed) / 0.5;
                        sow_ui::ui::animation::spring_overshoot(anim_t).clamp(0.0, 1.2)
                    } else {
                        1.0
                    };
                    let mut gpu_rendered = false;
                    if let Some(ref mut tr) = self.gfx.text_renderer {
                        gpu_rendered = true;
                        let color_arr = text_color.to_array().map(|v| v as f32 / 255.0);
                        let outline_color_arr = [0.0f32, 0.0, 0.0, alpha as f32 / 255.0];

                        let dev = sow_ui_kit::theme::dev_config::DevConfig::get();
                        let face_dilate = dev.font_face_dilate * sf;
                        let outline_thickness = dev.font_outline_thickness * sf;
                        let shadow_y = dev.font_shadow_y * sf;
                        let underlay_softness = dev.font_underlay_softness * sf;
                        let char_spacing = dev.font_char_spacing;
                        let font_size_scale = dev.font_size_scale;
                        let raw_emoji_scale = dev.emoji_size_scale;
                        let emoji_scale = if notice.text.contains('⚔') {
                            raw_emoji_scale * 0.65
                        } else {
                            raw_emoji_scale
                        };

                        let settings = crate::render::gpu::TmpFontSettings {
                            face_dilate,
                            outline_thickness,
                            underlay_offset_y: shadow_y,
                            underlay_softness,
                        };

                        tr.push_string(
                            &notice.text,
                            [screen_x * sf, screen_y * sf],
                            visual_config.gold_reward_notice_font_size
                                * bounce_scale
                                * font_size_scale
                                * sf,
                            (color_arr, outline_color_arr),
                            settings,
                            (0.5, char_spacing, emoji_scale),
                        );
                    }

                    if !gpu_rendered {
                        // Quantize to whole pixels for egui glyph atlas cache reuse
                        let font_size = (visual_config.gold_reward_notice_font_size * bounce_scale)
                            .round()
                            .max(1.0);

                        let font_id = egui::FontId::proportional(font_size);
                        sow_ui_kit::widgets::paint_emoji_text_at(
                            &middle_painter,
                            pos,
                            egui::Align2::CENTER_CENTER,
                            &notice.text,
                            font_id,
                            text_color,
                            true,
                        );
                    }
                }
                true
            });

            // Restore the player colors back to UI state to preserve the pre-allocated capacity
            self.ui.cached_player_colors = player_colors;
        }
    }
}
