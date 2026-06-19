use std::sync::Arc;

/// Paper-map label ink (off-white, not pure white).
pub const NAMEPLATE_FILL: egui::Color32 = egui::Color32::BLACK;

pub fn nameplate_matte_player_rgb(rgb: [f32; 3]) -> egui::Color32 {
    let y = 0.299_f64 * rgb[0] as f64 + 0.587 * rgb[1] as f64 + 0.114 * rgb[2] as f64;
    let sat = 0.58_f64;
    let mut r = y + (rgb[0] as f64 - y) * sat;
    let mut g = y + (rgb[1] as f64 - y) * sat;
    let mut b = y + (rgb[2] as f64 - y) * sat;
    r = (r * 0.92).clamp(0.12, 0.70);
    g = (g * 0.92).clamp(0.12, 0.70);
    b = (b * 0.92).clamp(0.12, 0.70);
    egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Brightens any player colors that are too dark for clean font rendering on the map,
/// ensuring a minimum relative luminance of 0.60 for optimal legibility.
pub fn ensure_readable_nameplate_color(rgb: [f32; 3]) -> egui::Color32 {
    let lum = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    let target_lum = 0.60;
    let factor = if lum < target_lum {
        (target_lum - lum) / (1.0 - lum).max(0.001)
    } else {
        0.0
    };
    let r = rgb[0] + (1.0 - rgb[0]) * factor;
    let g = rgb[1] + (1.0 - rgb[1]) * factor;
    let b = rgb[2] + (1.0 - rgb[2]) * factor;
    egui::Color32::from_rgb(
        (r * 255.0).round().clamp(0.0, 255.0) as u8,
        (g * 255.0).round().clamp(0.0, 255.0) as u8,
        (b * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

/// Draw the text galley directly with high performance (no outline).
pub fn paint_nameplate_galley(painter: &egui::Painter, pos: egui::Pos2, galley: Arc<egui::Galley>) {
    if !galley.is_empty() {
        painter.galley(pos, galley, NAMEPLATE_FILL);
    }
}

pub fn paint_glow_nameplate_galley(
    painter: &egui::Painter,
    pos: egui::Pos2,
    galley: Arc<egui::Galley>,
    base_color: egui::Color32,
) {
    paint_glow_nameplate_galley_with_ref(painter, pos, galley, base_color, None);
}

pub fn paint_glow_nameplate_galley_with_ref(
    painter: &egui::Painter,
    pos: egui::Pos2,
    galley: Arc<egui::Galley>,
    base_color: egui::Color32,
    reference_height: Option<f32>,
) {
    if galley.is_empty() {
        return;
    }
    let style = sow_ui::ui::theme::NAMEPLATE;
    sow_ui::ui::theme::text_glow::paint_glow_galley(
        painter,
        pos,
        galley,
        base_color,
        style,
        reference_height,
    );
}

pub fn name_label_size(painter: &egui::Painter, name: &str, font_id: &egui::FontId) -> egui::Vec2 {
    sow_ui::widgets::measure_emoji_text(painter, name, font_id)
}

pub fn paint_flat_name_label(
    painter: &egui::Painter,
    pos: egui::Pos2,
    name: &str,
    font_id: egui::FontId,
    color: egui::Color32,
) {
    sow_ui::widgets::paint_emoji_text_at(
        painter,
        pos,
        egui::Align2::LEFT_TOP,
        name,
        font_id,
        color,
        false,
    );
}

pub fn paint_glow_name_label(
    painter: &egui::Painter,
    pos: egui::Pos2,
    name: &str,
    font_id: egui::FontId,
    color: egui::Color32,
) {
    sow_ui::widgets::paint_emoji_text_at(
        painter,
        pos,
        egui::Align2::LEFT_TOP,
        name,
        font_id,
        color,
        true,
    );
}

pub fn layout_nameplate_troops_galley(
    painter: &egui::Painter,
    font_id: egui::FontId,
    troops_str: &str,
) -> Arc<egui::Galley> {
    painter.layout_no_wrap(troops_str.to_owned(), font_id, NAMEPLATE_FILL)
}

pub fn troops_icon_size(font_id: &egui::FontId) -> f32 {
    font_id.size * 1.15
}

pub fn troops_row_width(troops_galley: &egui::Galley, font_id: &egui::FontId) -> f32 {
    troops_icon_size(font_id) + 3.0 + troops_galley.rect.width()
}

pub fn paint_glow_troops_row(
    painter: &egui::Painter,
    pos: egui::Pos2,
    troops_galley: Arc<egui::Galley>,
    font_id: &egui::FontId,
    base_color: egui::Color32,
    reference_height: Option<f32>,
) {
    let icon_size = troops_icon_size(font_id);
    let icon_rect = egui::Rect::from_min_size(pos, egui::vec2(icon_size, icon_size));
    sow_ui::widgets::try_paint_emoji(painter, "⚔", icon_rect, base_color);
    let text_pos = pos + egui::vec2(icon_size + 3.0, 0.0);
    paint_glow_nameplate_galley_with_ref(
        painter,
        text_pos,
        troops_galley,
        base_color,
        reference_height,
    );
}
