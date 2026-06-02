use crate::app::SowApp;
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
        if self.ui.tutorial_completed {
            return;
        }

        let lang = self.ui.app.settings_state.language;
        let strings = &sow_i18n::get(lang).tutorial;

        // Auto-advance tutorial logic based on snapshot
        if let Some(snap) = &self.sim.current_snapshot {
            let my_id = self.sim.my_player_id.unwrap_or(0);
            if let Some(me) = snap.players.iter().find(|p| p.id == my_id) {
                if self.ui.tutorial_step == TutorialStep::Expansion && me.tile_count > 1 {
                    self.ui.tutorial_step = TutorialStep::Combat;
                } else if self.ui.tutorial_step == TutorialStep::Combat
                    && snap.attacks.iter().any(|a| a.owner_id == my_id)
                {
                    self.ui.tutorial_step = TutorialStep::Complete;
                    self.mark_tutorial_completed();
                }
            }
        }

        let (title, desc) = match self.ui.tutorial_step {
            TutorialStep::Welcome => (
                strings.step_welcome_title.as_str(),
                strings.step_welcome_desc.as_str(),
            ),
            TutorialStep::Expansion => (
                strings.step_expansion_title.as_str(),
                strings.step_expansion_desc.as_str(),
            ),
            TutorialStep::Combat => (
                strings.step_combat_title.as_str(),
                strings.step_combat_desc.as_str(),
            ),
            TutorialStep::Complete => (
                strings.step_complete_title.as_str(),
                strings.step_complete_desc.as_str(),
            ),
        };

        let mut next_clicked = false;

        egui::Window::new(&strings.commander_title)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_TOP, [0.0, 50.0])
            .frame(
                egui::Frame::window(&ctx.global_style())
                    .fill(Color32::from_rgba_premultiplied(10, 30, 60, 240))
                    .inner_margin(15.0)
                    .corner_radius(8.0),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("ℹ")
                            .color(Color32::LIGHT_BLUE)
                            .font(FontId::proportional(24.0)),
                    );
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(title)
                                .color(Color32::WHITE)
                                .font(FontId::proportional(18.0))
                                .strong(),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(desc)
                                .color(Color32::LIGHT_GRAY)
                                .font(FontId::proportional(16.0)),
                        );
                    });

                    ui.add_space(10.0);

                    if self.ui.tutorial_step == TutorialStep::Welcome
                        || self.ui.tutorial_step == TutorialStep::Complete
                    {
                        let btn_text = if self.ui.tutorial_step == TutorialStep::Welcome {
                            &strings.btn_next
                        } else {
                            &strings.btn_finish
                        };
                        if ui
                            .button(RichText::new(btn_text).color(Color32::WHITE))
                            .clicked()
                        {
                            next_clicked = true;
                        }
                    } else if ui
                        .button(
                            RichText::new(&strings.btn_skip)
                                .color(Color32::GRAY)
                                .size(12.0),
                        )
                        .clicked()
                    {
                        self.ui.tutorial_step = TutorialStep::Complete;
                        self.mark_tutorial_completed();
                    }
                });
            });

        if next_clicked {
            if self.ui.tutorial_step == TutorialStep::Welcome {
                self.ui.tutorial_step = TutorialStep::Expansion;
            } else if self.ui.tutorial_step == TutorialStep::Complete {
                self.mark_tutorial_completed();
            }
        }
    }

    fn mark_tutorial_completed(&mut self) {
        self.ui.tutorial_completed = true;
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
            let path = crate::paths::tutorial_completed_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, "true");
        }
    }
}
