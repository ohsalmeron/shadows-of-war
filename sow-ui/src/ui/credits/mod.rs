use crate::UiAction;
use crate::ui::theme::{accent_solo_cyan, text_secondary};
use egui::{Align, Color32, Layout, RichText, Stroke};
use sow_lang::Language;

pub fn draw(root_ui: &mut egui::Ui, is_open: bool, lang: Language) -> Option<UiAction> {
    if !is_open {
        return None;
    }

    let strings = &sow_lang::get(lang).credits;
    let mut action = None;
    let compact = root_ui.ctx().content_rect().width() < 768.0;
    let panel_w = if compact {
        root_ui.ctx().content_rect().width() - 32.0
    } else {
        560.0
    };

    let screen_rect = root_ui.ctx().content_rect();
    root_ui
        .ctx()
        .layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("credits_scrim"),
        ))
        .rect_filled(
            screen_rect,
            0.0,
            Color32::from_black_alpha(200),
        );

    egui::Window::new("credits_modal")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .fixed_size(egui::vec2(panel_w, 0.0))
        .frame(crate::ui::theme::standard_panel_frame(compact))
        .show(root_ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                crate::ui::theme::outlined_label(
                    ui,
                    &strings.title,
                    egui::FontId::proportional(if compact { 22.0 } else { 28.0 }),
                    Color32::WHITE,
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("✖").size(20.0).color(text_secondary()),
                            )
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE),
                        )
                        .clicked()
                    {
                        action = Some(UiAction::ToggleCredits);
                    }
                });
            });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);

            let body_size = if compact { 13.0 } else { 14.0 };
            let lines = [
                &strings.copyright_sow,
                &strings.license_agpl,
                &strings.based_on,
                &strings.maps_cc,
                &strings.notice,
                &strings.source,
                &strings.privacy,
            ];

            for line in lines {
                ui.label(
                    RichText::new(line.as_str())
                        .size(body_size)
                        .color(text_secondary()),
                );
                ui.add_space(6.0);
            }

            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                let close_btn = crate::widgets::ThemeButton::new(&strings.close)
                    .style(crate::widgets::ThemeButtonStyle::Primary)
                    .min_size(egui::vec2(
                        if compact { ui.available_width() } else { 180.0 },
                        44.0,
                    ));
                if ui.add(close_btn).clicked() {
                    action = Some(UiAction::ToggleCredits);
                }
            });

            ui.add_space(4.0);
            ui.label(
                RichText::new(&strings.splash_line)
                    .size(11.0)
                    .color(accent_solo_cyan()),
            );
        });

    action
}
