use crate::ui::theme::palette;
use crate::UiAction;
use egui::{Color32, RichText};
use sow_i18n::LegalDocument;

pub fn draw(
    root_ui: &mut egui::Ui,
    doc: &LegalDocument,
    is_open: bool,
    close_action: UiAction,
    modal_key: &str,
    reduced_motion: bool,
) -> Option<UiAction> {
    let mut open = is_open;
    let compact = crate::ui::theme::compact_viewport(root_ui.ctx());
    let mut action = None;

    let res = crate::ui::theme::draw_standard_modal(
        root_ui.ctx(),
        &mut open,
        modal_key,
        &doc.title,
        &doc.close,
        reduced_motion,
        |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(&doc.updated)
                        .font(crate::ui::theme::font_regular(if compact {
                            12.0
                        } else {
                            13.0
                        }))
                        .color(palette::text_muted()),
                )
                .wrap(),
            );
            ui.add_space(8.0);

            let body_size = if compact { 13.0 } else { 14.0 };
            let body = |text: &str| {
                RichText::new(text)
                    .font(crate::ui::theme::font_regular(body_size))
                    .color(palette::text_muted())
            };
            let link = |text: &str| {
                RichText::new(text)
                    .font(crate::ui::theme::font_regular(body_size))
                    .color(palette::neon_cyan())
            };
            let heading_size = if compact { 15.0 } else { 16.0 };

            for section in &doc.sections {
                ui.add(
                    egui::Label::new(
                        RichText::new(&section.heading)
                            .font(crate::ui::theme::font_regular(heading_size))
                            .color(Color32::WHITE),
                    )
                    .wrap(),
                );
                ui.add_space(4.0);
                for paragraph in &section.paragraphs {
                    ui.add(egui::Label::new(body(paragraph)).wrap());
                    ui.add_space(4.0);
                }
                for bullet in &section.bullets {
                    ui.add(egui::Label::new(body(&format!("•  {}", bullet))).wrap());
                    ui.add_space(2.0);
                }
                if !section.links.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        for (i, link_item) in section.links.iter().enumerate() {
                            if i > 0 {
                                ui.add_space(4.0);
                            }
                            if link_item.url.starts_with('#') {
                                if ui.link(link(&link_item.label)).clicked() {
                                    match link_item.url.as_str() {
                                        "#credits" => action = Some(UiAction::ToggleCredits),
                                        "#privacy" => action = Some(UiAction::TogglePrivacy),
                                        "#terms" => action = Some(UiAction::ToggleTerms),
                                        "#settings" => action = Some(UiAction::ToggleSettings),
                                        _ => {}
                                    }
                                }
                            } else {
                                ui.hyperlink_to(link(&link_item.label), &link_item.url);
                            }
                        }
                    });
                    ui.add_space(4.0);
                }
                ui.add_space(8.0);
            }
        },
    );

    if res.is_some() && !open {
        action = Some(close_action);
    }
    action
}
