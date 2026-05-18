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
                            let toggle_btn = egui::Button::new(RichText::new("🏆").size(18.0).color(Color32::from_gray(200)))
                                .fill(Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE);
                            if ui.add(toggle_btn).on_hover_text("Leaderboard").clicked() {
                                self.ui.show_leaderboard = !self.ui.show_leaderboard;
                            }

                            ui.add_space(2.0);

                            let dev_btn = egui::Button::new(RichText::new("🛠").size(18.0).color(Color32::from_gray(200)))
                                .fill(Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE);
                            if ui.add(dev_btn).on_hover_text("Dev Utils").clicked() {
                                self.ui.show_dev_sidebar = !self.ui.show_dev_sidebar;
                            }
                        });

                        if self.ui.show_leaderboard {
                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(4.0);
                            
                            let win_pct = self.sim.engine.as_ref()
                                .map(|e| e.state.config.map_control_win_percentage)
                                .unwrap_or(0.60);
                                
                            ui.vertical_centered(|ui| {
                                egui::Frame::new()
                                    .fill(sow_ui::ui::theme::nickname_field_bg())
                                    .stroke(egui::Stroke::new(1.0_f32, sow_ui::ui::theme::accent_ranked_gold()))
                                    .corner_radius(8.0)
                                    .inner_margin(egui::Margin::symmetric(16.0, 8.0))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(format!("👑 Domination Victory: Control {:.0}% of Map", win_pct * 100.0))
                                                .color(sow_ui::ui::theme::accent_ranked_gold())
                                                .size(14.0)
                                                .strong()
                                        );
                                    });
                            });
                                
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

                        if self.ui.show_dev_sidebar {
                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(4.0);
                            ui.style_mut().spacing.slider_width = 100.0;
                            ui.style_mut().spacing.item_spacing = Vec2::new(4.0, 4.0);

                            let mut thick = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::new("dev_thickness"), || 0.65f32));
                            let mut dark = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::new("dev_darkness"), || 0.40f32));
                            let mut s_thick = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::new("dev_shore_thickness"), || 0.0f32));
                            let mut s_dark = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::new("dev_shore_darkness"), || 0.47f32));
                            let mut roundness = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::new("dev_roundness"), || 1.0f32));
                            
                            ui.add(egui::Slider::new(&mut thick, 0.0..=1.0).text("Border Thk"));
                            ui.add(egui::Slider::new(&mut dark, 0.0..=1.0).text("Border Drk"));
                            ui.add(egui::Slider::new(&mut s_thick, 0.0..=1.0).text("Shore Thk"));
                            ui.add(egui::Slider::new(&mut s_dark, 0.0..=1.0).text("Shore Drk"));
                            ui.add(egui::Slider::new(&mut roundness, 0.0..=1.0).text("Roundness"));
                            
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_thickness"), thick));
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_darkness"), dark));
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_shore_thickness"), s_thick));
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_shore_darkness"), s_dark));
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_roundness"), roundness));
                        }
                    });
            });
    }
}
