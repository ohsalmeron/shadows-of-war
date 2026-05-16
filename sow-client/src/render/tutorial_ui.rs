use crate::app_state::SowApp;
use egui::{Align2, Color32, FontId, RichText};

#[derive(Debug, Clone, PartialEq)]
pub enum TutorialStep {
    Welcome,
    Expansion,
    Combat,
    Complete,
}

impl SowApp {
    pub(crate) fn render_tutorial_ui(&mut self, ctx: &egui::Context) {
        if self.tutorial_completed {
            return;
        }

        // Auto-advance tutorial logic based on snapshot
        if let Some(snap) = &self.current_snapshot {
            let my_id = self.my_player_id.unwrap_or(0);
            if let Some(me) = snap.players.iter().find(|p| p.id == my_id) {
                if self.tutorial_step == TutorialStep::Expansion && me.tile_count > 1 {
                    self.tutorial_step = TutorialStep::Combat;
                } else if self.tutorial_step == TutorialStep::Combat && snap.attacks.iter().any(|a| a.owner_id == my_id) {
                    self.tutorial_step = TutorialStep::Complete;
                    self.mark_tutorial_completed();
                }
            }
        }

        let (title, desc) = match self.tutorial_step {
            TutorialStep::Welcome => ("Welcome Commander", "Let's establish your empire. Your base is the single tile you own."),
            TutorialStep::Expansion => ("Expand Territory", "Tap on a neutral (empty) tile next to your border to expand."),
            TutorialStep::Combat => ("Launch an Attack", "Enemies approach. Drag from your territory into an enemy tile to attack!"),
            TutorialStep::Complete => ("You are ready", "Conquer the world! Good luck."),
        };

        let mut next_clicked = false;

        egui::Window::new("Tutorial")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_TOP, [0.0, 50.0])
            .frame(egui::Frame::window(&ctx.global_style()).fill(Color32::from_rgba_premultiplied(10, 30, 60, 240)).inner_margin(15.0).corner_radius(8.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("ℹ").color(Color32::LIGHT_BLUE).font(FontId::proportional(24.0)));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(title).color(Color32::WHITE).font(FontId::proportional(18.0)).strong());
                        ui.add_space(4.0);
                        ui.label(RichText::new(desc).color(Color32::LIGHT_GRAY).font(FontId::proportional(16.0)));
                    });
                    
                    ui.add_space(10.0);
                    
                    if self.tutorial_step == TutorialStep::Welcome || self.tutorial_step == TutorialStep::Complete {
                        let btn_text = if self.tutorial_step == TutorialStep::Welcome { "Next" } else { "Finish" };
                        if ui.button(RichText::new(btn_text).color(Color32::WHITE)).clicked() {
                            next_clicked = true;
                        }
                    } else {
                        // Skip button for the action steps
                        if ui.button(RichText::new("Skip").color(Color32::GRAY).size(12.0)).clicked() {
                            self.tutorial_step = TutorialStep::Complete;
                            self.mark_tutorial_completed();
                        }
                    }
                });
            });

        if next_clicked {
            if self.tutorial_step == TutorialStep::Welcome {
                self.tutorial_step = TutorialStep::Expansion;
            } else if self.tutorial_step == TutorialStep::Complete {
                self.mark_tutorial_completed();
            }
        }
    }

    fn mark_tutorial_completed(&mut self) {
        self.tutorial_completed = true;
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item("sow_tutorial_completed", "true");
                }
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = std::fs::write("sow_tutorial_completed.txt", "true");
        }
    }
}
