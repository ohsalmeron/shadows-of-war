use egui::{Color32, Context, Frame, RichText};

#[derive(Debug, Clone, PartialEq)]
pub enum SplashJob {
    Boot,
    EnterGame,
    ExitGame,
    Reconnect,
}

pub struct SplashState {
    pub job: SplashJob,
    pub status_text: String,
    pub progress: f32, // 0.0 to 1.0
    pub frames_drawn: u32,
    pub thumbnail: Option<egui::TextureHandle>,
    pub done: bool,
}

impl Default for SplashState {
    fn default() -> Self {
        Self {
            job: SplashJob::Boot,
            status_text: "Initializing...".to_string(),
            progress: 0.0,
            frames_drawn: 0,
            thumbnail: None,
            done: false,
        }
    }
}

pub fn draw(ctx: &Context, state: &mut SplashState) {
    state.frames_drawn += 1;
    #[allow(deprecated)]
    egui::CentralPanel::default()
        .frame(Frame::default().fill(crate::ui::theme::menu_backdrop()))
        .show(ctx, |ui| {
            if let Some(texture) = &state.thumbnail {
                ui.painter().image(
                    texture.id(),
                    ui.max_rect(),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::from_white_alpha(100),
                );
            }
            let screen_w = ui.available_width();
            let is_mobile = screen_w < 600.0;
            let bar_width = if is_mobile { screen_w * 0.8 } else { 400.0 };
            
            // Bottom UI (Loading Bar)
            egui::TopBottomPanel::bottom("loading_bottom_panel")
                .frame(Frame::none().inner_margin(egui::Margin::same(40)))
                .show_inside(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        if state.progress == 0.0 {
                            ui.add(egui::Spinner::new().size(if is_mobile { 24.0 } else { 32.0 }).color(Color32::from_rgb(255, 200, 100)));
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new(&state.status_text)
                                    .size(if is_mobile { 16.0 } else { 20.0 })
                                    .color(egui::Color32::WHITE)
                            );
                        } else {
                            let progress = state.progress;
                            ui.add(
                                egui::ProgressBar::new(progress)
                                    .desired_width(bar_width)
                                    .fill(Color32::from_rgb(255, 180, 50))
                                    .text(RichText::new(format!("{}% Complete", (progress * 100.0) as u32)).color(Color32::BLACK))
                            );
                            ui.add_space(10.0);
                            
                            ui.label(
                                RichText::new(&state.status_text)
                                    .size(if is_mobile { 14.0 } else { 18.0 })
                                    .color(egui::Color32::from_gray(220))
                            );
                        }
                    });
                });

            // Center UI (Title)
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("SHADOWS OF WAR")
                            .strong()
                            .size(if is_mobile { 36.0 } else { 64.0 })
                            .color(Color32::from_rgb(255, 230, 200))
                    );
                });
            });
        });
}
