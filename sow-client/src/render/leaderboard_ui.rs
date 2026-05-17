use egui::{Align2, Color32, RichText, Vec2, Window};
use crate::app::SowApp;

impl SowApp {
    pub fn render_leaderboard(&mut self, ctx: &egui::Context) {
        // 1. Throttle the leaderboard update to once per second
        self.leaderboard_timer -= self.raw_input.predicted_dt;
        if self.leaderboard_timer <= 0.0 {
            self.leaderboard_timer = 1.0; // Reset timer
            
            if let Some(snap) = &self.current_snapshot {
                let mut new_board = Vec::new();
                for p in &snap.players {
                    if p.alive {
                        let display_name = if p.name.is_empty() {
                            if p.id >= 200 { format!("Tribe {}", p.id - 199) } 
                            else { format!("Nation {}", p.id - 103) }
                        } else {
                            p.name.clone()
                        };
                        new_board.push((p.id, display_name, p.tile_count, p.troops as f64));
                    }
                }
                // O(N log N) extremely fast sort for < 200 elements
                // Sort descending by tile count
                new_board.sort_unstable_by(|a, b| b.2.cmp(&a.2));
                self.cached_leaderboard = new_board;
            }
        }

        // 2. Render the Egui window
        let mut show_leaderboard = self.show_leaderboard;
        
        Window::new("Leaderboard")
            .open(&mut show_leaderboard)
            .anchor(Align2::LEFT_TOP, Vec2::new(10.0, 48.0))
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
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
                            let total_land_tiles = self.current_snapshot
                                .as_ref()
                                .map(|s| s.total_land_tiles)
                                .unwrap_or(1)
                                .max(1);

                            // Data rows
                            for (i, (id, name, tiles, troops)) in self.cached_leaderboard.iter().enumerate() {
                                // Highlight the player's own row
                                let is_me = Some(*id) == self.my_player_id;
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
            });
            
        self.show_leaderboard = show_leaderboard;
    }
}
