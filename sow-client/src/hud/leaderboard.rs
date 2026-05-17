use egui::{Align2, Color32, RichText, Vec2};
use crate::app::SowApp;

impl SowApp {
    pub fn render_leaderboard(&mut self, ctx: &egui::Context) {
        // 1. Throttle the leaderboard update to once per second
        self.ui.leaderboard_timer -= self.ui.raw_input.predicted_dt;
        if self.ui.leaderboard_timer <= 0.0 {
            self.ui.leaderboard_timer = 1.0; // Reset timer
            
            if let Some(snap) = &self.sim.current_snapshot {
                let mut new_board = Vec::new();
                for p in &snap.players {
                    if p.alive {
                        let display_name = if p.name.is_empty() {
                            if p.id >= 200 { format!("Tribe {}", p.id - 199) } 
                            else { format!("Nation {}", p.id - 103) }
                        } else {
                            p.name.clone()
                        };
                        new_board.push((p.id, display_name, p.tile_count, p.troops));
                    }
                }
                // O(N log N) extremely fast sort for < 200 elements
                // Sort descending by tile count
                new_board.sort_unstable_by(|a, b| b.2.cmp(&a.2));
                self.ui.cached_leaderboard = new_board;
            }
        }

        egui::Area::new(egui::Id::new("leaderboard_area"))
            .anchor(Align2::LEFT_TOP, Vec2::new(12.0, 12.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(sow_ui::ui::theme::panel_bg_transparent())
                    .stroke(egui::Stroke::new(1.0_f32, sow_ui::ui::theme::nickname_field_border()))
                    .corner_radius(12.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        // Toggle Button Row
                        ui.horizontal(|ui| {
                            let icon = if self.ui.show_leaderboard { "▼" } else { "▶" };
                            let toggle_btn = egui::Button::new(RichText::new(format!("{} 🏆 Leaderboard", icon)).size(16.0).color(Color32::from_gray(220)))
                                .fill(Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE);
                            if ui.add(toggle_btn).clicked() {
                                self.ui.show_leaderboard = !self.ui.show_leaderboard;
                            }
                        });

                        if self.ui.show_leaderboard {
                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(4.0);
                            ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);
                            
                            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                                egui::Grid::new("leaderboard_grid")
                                    .num_columns(5)
                                    .spacing([10.0, 8.0])
                                    .striped(true)
                                    .show(ui, |ui| {
                                        // Headers
                                        ui.label(RichText::new("#").strong());
                                        ui.label(RichText::new("Name").strong());
                                        ui.label(RichText::new("Tiles").strong());
                                        ui.label(RichText::new("Troops").strong());
                                        ui.label(RichText::new("Control").strong());
                                        ui.end_row();

                                        // Get total land tiles from snapshot
                                        let total_land_tiles = self.sim.current_snapshot
                                            .as_ref()
                                            .map(|s| s.total_land_tiles)
                                            .unwrap_or(1)
                                            .max(1);

                                        // Data rows
                                        for (i, (id, name, tiles, troops)) in self.ui.cached_leaderboard.iter().enumerate() {
                                            // Highlight the player's own row
                                            let is_me = Some(*id) == self.sim.my_player_id;
                                            let color = if is_me { Color32::YELLOW } else { Color32::WHITE };
                                            
                                            let control_pct = (*tiles as f32 / total_land_tiles as f32) * 100.0;
                                            
                                            ui.label(RichText::new(format!("{}", i + 1)).color(color));
                                            ui.label(RichText::new(name).color(color));
                                            ui.label(RichText::new(format!("{}", tiles)).color(color));
                                            ui.label(RichText::new(format!("{:.0}", troops)).color(color));
                                            ui.label(RichText::new(format!("{:.1}%", control_pct)).color(color));
                                            ui.end_row();
                                        }
                                    });
                            });
                        }
                    });
            });
    }
}
