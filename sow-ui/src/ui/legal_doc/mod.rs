use crate::ui::theme::palette;
use crate::UiAction;
use egui::{Align, Color32, Layout, RichText, ScrollArea};
use sow_i18n::LegalDocument;

pub fn draw(
    root_ui: &mut egui::Ui,
    doc: &LegalDocument,
    is_open: bool,
    close_action: UiAction,
    modal_key: &str,
    reduced_motion: bool,
) -> Option<UiAction> {
    if !is_open {
        return None;
    }

    let mut action = None;
    let compact = crate::ui::theme::compact_viewport(root_ui.ctx());
    let panel_w = if compact {
        root_ui.ctx().input(|i| i.content_rect()).width() - 32.0
    } else {
        520.0
    };

    let progress = root_ui.ctx().animate_bool_with_time(
        egui::Id::new(format!("{modal_key}_animation_progress")),
        is_open,
        crate::ui::theme::anim_duration(reduced_motion),
    );
    if progress <= 0.01 {
        return None;
    }

    let screen_rect = root_ui.ctx().input(|i| i.content_rect());
    root_ui
        .ctx()
        .layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new(format!("{modal_key}_scrim")),
        ))
        .rect_filled(
            screen_rect,
            0.0,
            Color32::from_black_alpha((200.0 * progress) as u8),
        );

    let y_offset = if is_open {
        let t = progress;
        if t >= 1.0 {
            0.0
        } else {
            -80.0 * (1.0 - t)
        }
    } else {
        0.0
    };

    egui::Window::new(format!("{modal_key}_modal"))
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, y_offset))
        .fixed_size(egui::vec2(panel_w, if compact { 460.0 } else { 420.0 }))
        .frame(crate::ui::theme::standard_panel_frame(compact))
        .show(root_ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                crate::ui::theme::outlined_label(
                    ui,
                    &doc.title,
                    egui::FontId::proportional(if compact { 20.0 } else { 24.0 }),
                    Color32::WHITE,
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if crate::ui::theme::modal_close_button(ui).clicked() {
                        action = Some(close_action.clone());
                    }
                });
            });

            ui.add_space(6.0);
            ui.label(
                RichText::new(&doc.updated)
                    .size(if compact { 12.0 } else { 13.0 })
                    .color(palette::text_muted()),
            );
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            let body_size = if compact { 13.0 } else { 14.0 };
            let body = |text: &str| RichText::new(text).size(body_size).color(palette::text_muted());
            let link = |text: &str| {
                RichText::new(text)
                    .size(body_size)
                    .color(palette::neon_cyan())
            };
            let heading_size = if compact { 15.0 } else { 16.0 };

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for section in &doc.sections {
                        ui.label(
                            RichText::new(&section.heading)
                                .size(heading_size)
                                .color(Color32::WHITE),
                        );
                        ui.add_space(4.0);
                        for paragraph in &section.paragraphs {
                            ui.label(body(paragraph));
                            ui.add_space(4.0);
                        }
                        for bullet in &section.bullets {
                            ui.horizontal(|ui| {
                                ui.label(body("•"));
                                ui.label(body(bullet));
                            });
                            ui.add_space(2.0);
                        }
                        if !section.links.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                for (i, link_item) in section.links.iter().enumerate() {
                                    if i > 0 {
                                        ui.add_space(4.0);
                                    }
                                    ui.hyperlink_to(link(&link_item.label), &link_item.url);
                                }
                            });
                            ui.add_space(4.0);
                        }
                        ui.add_space(8.0);
                    }
                });

            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                let close_btn = crate::widgets::ThemeButton::new(&doc.close)
                    .style(crate::widgets::ThemeButtonStyle::Primary)
                    .min_size(egui::vec2(
                        if compact { ui.available_width() } else { 160.0 },
                        40.0,
                    ));
                if ui.add(close_btn).clicked() {
                    action = Some(close_action);
                }
            });
        });

    action
}
