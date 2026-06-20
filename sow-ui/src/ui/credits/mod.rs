use crate::UiAction;
use egui::RichText;
use sow_i18n::Language;
use sow_ui_kit::theme::palette;

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
    let mut open = is_open;
    let compact = sow_ui_kit::theme::compact_viewport(root_ui.ctx());
    let strings = &sow_i18n::get(lang).credits;
    let settings_strings = &sow_i18n::get(lang).settings;
    let mut action = None;

    let res = sow_ui_kit::theme::draw_standard_modal(
        root_ui.ctx(),
        &mut open,
        "credits",
        &strings.title,
        &strings.close,
        reduced_motion,
        |ui| {
            let body_size = if compact { 13.0 } else { 14.0 };
            let body = |text: &str| {
                RichText::new(text)
                    .size(body_size)
                    .color(palette::text_muted())
            };
            let link = |text: &str| {
                RichText::new(text)
                    .size(body_size)
                    .color(palette::neon_cyan())
            };

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
        },
    );

    if res.is_some() && !open {
        action = Some(UiAction::ToggleCredits);
    }
    action
}
