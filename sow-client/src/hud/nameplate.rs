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

/// Draw the text galley directly with high performance (no outline).
pub fn paint_nameplate_galley(painter: &egui::Painter, pos: egui::Pos2, galley: Arc<egui::Galley>) {
    if !galley.is_empty() {
        painter.galley(pos, galley, NAMEPLATE_FILL);
    }
}

/// Paint a pre-laid-out galley with premium outline. Zero layout cost.
fn paint_glow_galley(
    painter: &egui::Painter,
    pos: egui::Pos2,
    galley: Arc<egui::Galley>,
    base_color: egui::Color32,
    is_tribe: bool,
) {
    let black = egui::Color32::BLACK;

    if is_tribe {
        painter.galley_with_override_text_color(pos + egui::vec2(1.0, 1.0), galley.clone(), black);
        painter.galley_with_override_text_color(pos, galley, base_color);
        return;
    }

    // 2 dragged shadows + 4 diagonal outline + 1 core = 7 passes, zero layout cost
    for &dy in &[2.0, 4.0] {
        painter.galley_with_override_text_color(pos + egui::vec2(0.0, dy), galley.clone(), black);
    }
    for &(dx, dy) in &[(-1.5, -1.5), (1.5, -1.5), (-1.5, 1.5), (1.5, 1.5)] {
        painter.galley_with_override_text_color(pos + egui::vec2(dx, dy), galley.clone(), black);
    }
    painter.galley_with_override_text_color(pos, galley, base_color);
}

pub fn paint_glow_nameplate_galley(
    painter: &egui::Painter,
    pos: egui::Pos2,
    galley: Arc<egui::Galley>,
    base_color: egui::Color32,
    is_tribe: bool,
) {
    if !galley.is_empty() {
        paint_glow_galley(painter, pos, galley, base_color, is_tribe);
    }
}

pub fn layout_nameplate_name_galley(
    painter: &egui::Painter,
    font_id: egui::FontId,
    name: &str,
) -> Arc<egui::Galley> {
    painter.layout_no_wrap(name.to_owned(), font_id, NAMEPLATE_FILL)
}

pub fn layout_nameplate_troops_galley(
    painter: &egui::Painter,
    font_id: egui::FontId,
    troops_str: &str,
) -> Arc<egui::Galley> {
    let text = format!("⚔ {}", troops_str);
    painter.layout_no_wrap(text, font_id, NAMEPLATE_FILL)
}
