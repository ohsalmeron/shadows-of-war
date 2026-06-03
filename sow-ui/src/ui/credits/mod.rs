use crate::UiAction;
use crate::ui::theme::{accent_solo_cyan, text_secondary};
use egui::{Align, Color32, Layout, RichText, ScrollArea};
use sow_i18n::Language;

fn version_tag() -> String {
    format!("v{}", include_str!("../../../../.version").trim())
}

fn source_tag_url() -> String {
    format!(
        "https://github.com/ohsalmeron/shadows-of-war/tree/{}",
        version_tag()
    )
}

const GITHUB_REPO: &str = "https://github.com/ohsalmeron/shadows-of-war";

fn legal_blob_url(path: &str) -> String {
    format!("{GITHUB_REPO}/blob/{}/{}", version_tag(), path)
}

fn assets_license_url() -> String {
    legal_blob_url("docs/legal/LICENSE-ASSETS")
}

fn notice_url() -> String {
    legal_blob_url("docs/legal/NOTICE")
}

pub fn draw(
    root_ui: &mut egui::Ui,
    is_open: bool,
    lang: Language,
    reduced_motion: bool,
) -> Option<UiAction> {
    if !is_open {
        return None;
    }

    let strings = &sow_i18n::get(lang).credits;
    let settings_strings = &sow_i18n::get(lang).settings;
    let mut action = None;
    let compact = root_ui.ctx().content_rect().width() < 768.0;
    let panel_w = if compact {
        root_ui.ctx().content_rect().width() - 32.0
    } else {
        520.0
    };

    let progress = root_ui.ctx().animate_bool_with_time(
        egui::Id::new("credits_animation_progress"),
        is_open,
        crate::ui::theme::anim_duration(reduced_motion),
    );
    if progress <= 0.01 {
        return None;
    }

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

    egui::Window::new("credits_modal")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, y_offset))
        .fixed_size(egui::vec2(panel_w, if compact { 420.0 } else { 380.0 }))
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
                    if crate::ui::theme::modal_close_button(ui).clicked() {
                        action = Some(UiAction::ToggleCredits);
                    }
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            let body_size = if compact { 13.0 } else { 14.0 };
            let body = |text: &str| RichText::new(text).size(body_size).color(text_secondary());
            let link = |text: &str| RichText::new(text).size(body_size).color(accent_solo_cyan());

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for line in [
                        &strings.copyright_sow,
                        &strings.license_agpl,
                        &strings.based_on,
                    ] {
                        ui.label(body(line));
                        ui.add_space(8.0);
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.label(body(&strings.assets_license));
                        ui.add_space(4.0);
                        let assets_url = assets_license_url();
                        ui.hyperlink_to(link("LICENSE-ASSETS"), &assets_url);
                    });
                    ui.add_space(8.0);

                    let tag = version_tag();
                    let tag_url = source_tag_url();
                    ui.horizontal_wrapped(|ui| {
                        ui.label(body(&format!("{} ({}) ", strings.source_label, tag)));
                        ui.hyperlink_to(link(&tag_url), &tag_url);
                    });
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(body(&format!("{}: ", strings.privacy_label)));
                        if ui
                            .add(
                                egui::Button::new(link(&settings_strings.privacy_policy))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::NONE),
                            )
                            .clicked()
                        {
                            action = Some(UiAction::TogglePrivacy);
                        }
                    });
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(body(&strings.notice));
                        let notice = notice_url();
                        ui.hyperlink_to(link("NOTICE"), &notice);
                    });
                });

            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                let close_btn = crate::widgets::ThemeButton::new(&strings.close)
                    .style(crate::widgets::ThemeButtonStyle::Primary)
                    .min_size(egui::vec2(
                        if compact { ui.available_width() } else { 160.0 },
                        40.0,
                    ));
                if ui.add(close_btn).clicked() {
                    action = Some(UiAction::ToggleCredits);
                }
            });
        });

    action
}
