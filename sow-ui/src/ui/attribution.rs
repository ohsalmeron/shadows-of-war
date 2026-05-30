//! OpenFront AGPL §7 copyright preservation in visible UI locations.

use egui::{Align2, Color32, FontId};
use sow_lang::Language;

/// Draw minimal required attribution: "© OpenFront and Contributors".
pub fn draw_openfront_footer(ui: &egui::Ui, lang: Language) {
    let text = &sow_lang::get(lang).credits.openfront_footer;
    let rect = ui.ctx().content_rect();
    let compact = rect.width() < 768.0;
    let font = FontId::proportional(if compact { 9.0 } else { 10.0 });
    let pos = egui::pos2(rect.min.x + 10.0, rect.max.y - 8.0);
    ui.painter().text(
        pos,
        Align2::LEFT_BOTTOM,
        text.as_str(),
        font,
        Color32::from_rgba_unmultiplied(189, 189, 189, 200),
    );
}
