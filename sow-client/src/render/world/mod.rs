pub mod utils;
pub mod layer1_railroads;
pub mod layer2_fleets;
pub mod layer3_buildings;
pub mod layer4_5_effects;
pub mod layer6_projectiles;
pub mod layer7_preview;
pub mod layer8_nameplates;

pub use utils::*;

use crate::config::ClientVisualConfig;
use crate::hud::nameplate::*;
use crate::app::SowApp;

#[derive(Copy, Clone)]
pub(crate) struct VisPlayer<'a> {
    pub player: &'a sow_core::protocol::PlayerSnapshot,
    pub center: egui::Pos2,
    pub pc: egui::Color32,
    pub lod_presence: f32,
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
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("world_overlays"),
        ));
        let wall_secs = self.time.start_time.elapsed().as_secs_f64();
        let current_tick = self.sim.current_snapshot.as_ref().map(|s| s.tick).unwrap_or(0);

        // Configuration variables removed from GameConfig
        let dot_r = ClientVisualConfig::default().ui_lod_dot_radius;


        let mut visible_players = Vec::new();
        if let Some(snap) = &self.sim.current_snapshot {
            for player in &snap.players {
                if player.tile_count == 0 || !player.alive {
                    continue;
                }

                let avg_col = player.centroid_x;
                let avg_row = player.centroid_y;

                let target_cx = avg_col + 0.5;
                let target_cy = avg_row + 0.5;

                // Smooth position interpolation
                let pos = self
                    .ui
                    .label_positions
                    .entry(player.id)
                    .or_insert((target_cx, target_cy));
                let dx = target_cx - pos.0;
                let dy = target_cy - pos.1;
                let dist = (dx * dx + dy * dy).sqrt();
                let dt = self.ui.raw_input.predicted_dt;
                let smooth_factor = 1.0 - (-10.0 * dt).exp(); // Frame-rate independent
                if dist > 50.0 {
                    pos.0 = target_cx;
                    pos.1 = target_cy;
                } else if dist > 0.1 {
                    pos.0 += dx * smooth_factor;
                    pos.1 += dy * smooth_factor;
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
                // Map shader derives human tint from id, not `player.color`; match that for dots + ★.
                let rgb = if player.player_type == sow_core::player::PlayerType::Human {
                    sow_core::player::human_shader_territory_rgb(player.id)
                } else {
                    player.color
                };
                let pc = nameplate_matte_player_rgb(rgb);

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

                visible_players.push(VisPlayer {
                    player,
                    center,
                    pc,
                    lod_presence,
                });
            }

            let zoom_scaled = self.input.camera_zoom / sf;

            // Clean up expired upgrade animations once per frame
            let now = web_time::Instant::now();
            self.ui.active_upgrades.retain(|anim| now.duration_since(anim.start_time) < anim.duration);

            // Rebuild player colors only when the player list size changes
            let player_count = snap.players.len();
            if self.ui.cached_player_count != player_count {
                let max_pid = snap.players.iter().map(|p| p.id).max().unwrap_or(0) as usize;
                self.ui.cached_player_colors.resize(max_pid + 1, egui::Color32::GRAY);
                self.ui.cached_player_count = player_count;
            }
            for p in &snap.players {
                let id = p.id as usize;
                let rgb = if p.player_type == sow_core::player::PlayerType::Human {
                    sow_core::player::human_shader_territory_rgb(p.id)
                } else {
                    p.color
                };
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

            let terrain = self.gfx.map_renderer.as_ref().map(|mr| mr.terrain.as_slice()).unwrap_or(&[]);


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

            layer1_railroads::render(&mut self.ui, &self.sim, &self.input, &self.time, &self.gfx, &ctx_struct);
            layer2_fleets::render(&mut self.ui, &self.sim, &self.input, &self.time, &self.gfx, &ctx_struct);
            layer3_buildings::render(&mut self.ui, &self.sim, &self.input, &self.time, &self.gfx, &ctx_struct);
            layer4_5_effects::render(&mut self.ui, &self.sim, &self.input, &self.time, &self.gfx, &ctx_struct);
            layer6_projectiles::render(&mut self.ui, &self.sim, &self.input, &self.time, &self.gfx, &ctx_struct);
            layer7_preview::render(&mut self.ui, &self.sim, &self.input, &self.time, &self.gfx, &ctx_struct);
            layer8_nameplates::render(&mut self.ui, &self.sim, &self.input, &self.time, &self.gfx, &ctx_struct);

            // Restore the player colors back to UI state to preserve the pre-allocated capacity
            self.ui.cached_player_colors = player_colors;
        }
    }
}
