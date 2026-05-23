use egui::{Align2, Color32, FontId, RichText};
use sow_lang::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndgameAction {
    ExitToLobby,
    Spectate,
}

pub struct EndgameConfig<'a> {
    pub is_victory: bool,
    pub winner_name: Option<&'a str>,
    pub is_spectating: bool,
    pub show_spectate_btn: bool,
    pub flavor_index: usize,
}

#[allow(deprecated)]
pub fn draw(ctx: &egui::Context, config: &EndgameConfig, lang: Language) -> Option<EndgameAction> {
    let mut action = None;

    let strings = &sow_lang::get(lang).endgame;
    let screen_rect = ctx.content_rect();
    let compact = screen_rect.width() < 768.0;

    // Dim background
    egui::Area::new(egui::Id::new("endgame_dimmer"))
        .order(egui::Order::Background)
        .fixed_pos(egui::Pos2::ZERO)
        .show(ctx, |ui| {
            let rect = ctx.content_rect();
            ui.painter()
                .rect_filled(rect, 0.0, Color32::from_black_alpha(150));
        });

    let window = egui::Window::new("Endgame")
        .title_bar(false)
        .collapsible(false)
        .resizable(false);

    let window = if compact {
        window.fixed_size(screen_rect.size())
            .anchor(Align2::LEFT_TOP, [0.0, 0.0])
    } else {
        window.anchor(Align2::CENTER_CENTER, [0.0, 0.0])
    };

    window.frame(
        egui::Frame::window(&ctx.global_style())
            .fill(crate::ui::theme::panel_bg())
            .stroke(if compact {
                egui::Stroke::NONE
            } else {
                egui::Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border())
            })
            .corner_radius(if compact { 0.0 } else { 12.0 })
            .inner_margin(if compact { 24.0 } else { 30.0 }),
    )
    .show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            if compact {
                ui.add_space(screen_rect.height() * 0.15); // Push down on mobile
            }

            let (title_text, title_color, default_sub) = if config.is_victory {
                (strings.victory_title.as_str(), Color32::GOLD, strings.victory_subtitle.as_str())
            } else {
                (strings.defeat_title.as_str(), Color32::RED, strings.defeat_subtitle.as_str())
            };

            let subtitle = if let Some(winner) = config.winner_name {
                format!("{} emerged victorious.", winner)
            } else {
                let flavors = if config.is_victory {
                    &strings.victory_flavors
                } else {
                    &strings.defeat_flavors
                };
                let idx = config.flavor_index % flavors.len();
                flavors.get(idx).map(|s| s.as_str()).unwrap_or(default_sub).to_string()
            };

            crate::ui::theme::outlined_label(
                ui,
                title_text,
                FontId::proportional(if compact { 36.0 } else { 64.0 }),
                title_color,
            );
            ui.add_space(10.0);
            ui.label(
                RichText::new(&subtitle)
                    .color(Color32::LIGHT_GRAY)
                    .font(FontId::proportional(if compact { 16.0 } else { 24.0 })),
            );

            ui.add_space(if compact { 40.0 } else { 30.0 });

            let btn_color = if config.is_victory {
                Color32::from_rgb(40, 140, 40)
            } else {
                Color32::from_rgb(140, 40, 40)
            };

            let btn_size = if compact {
                egui::vec2(ui.available_width().min(320.0), 44.0)
            } else {
                egui::vec2(200.0, 50.0)
            };

            if ui
                .add_sized(
                    btn_size,
                    crate::widgets::ThemeButton::new(&strings.return_to_lobby)
                        .custom_text_color(Color32::WHITE)
                        .text_size(if compact { 16.0 } else { 20.0 })
                        .custom_fill(btn_color),
                )
                .clicked()
            {
                action = Some(EndgameAction::ExitToLobby);
            }

            if config.show_spectate_btn {
                ui.add_space(15.0);
                if ui
                    .add_sized(
                        btn_size,
                        crate::widgets::ThemeButton::new(&strings.spectate)
                            .custom_text_color(Color32::WHITE)
                            .text_size(if compact { 16.0 } else { 20.0 })
                            .custom_fill(Color32::from_rgb(60, 60, 60)),
                    )
                    .clicked()
                {
                    action = Some(EndgameAction::Spectate);
                }
            }
        });
    });

    action
}
