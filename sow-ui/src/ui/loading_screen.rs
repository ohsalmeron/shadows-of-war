use egui::{Color32, Context, Frame, RichText};

pub struct LoadingState {
    pub status_text: String,
    pub progress: f32, // 0.0 to 1.0
    pub frames_drawn: u32,
}

impl Default for LoadingState {
    fn default() -> Self {
        Self {
            status_text: "Initializing...".to_string(),
            progress: 0.0,
            frames_drawn: 0,
        }
    }
}

pub fn draw(ctx: &Context, state: &mut LoadingState) {
    state.frames_drawn += 1;
    #[allow(deprecated)]
    egui::CentralPanel::default()
        .frame(Frame::default().fill(crate::ui::theme::menu_backdrop()))
        .show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("SHADOWS OF WAR")
                            .strong()
                            .size(48.0)
                            .color(Color32::from_rgb(255, 230, 200))
                    );
                    ui.add_space(20.0);
                    
                    ui.add(egui::Spinner::new().size(32.0).color(Color32::from_rgb(100, 150, 255)));
                    ui.add_space(20.0);
                    
                    ui.label(
                        RichText::new(&state.status_text)
                            .size(20.0)
                            .color(egui::Color32::WHITE)
                    );
                });
            });
        });
}
