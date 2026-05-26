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

pub fn paint_glow_text(
    painter: &egui::Painter,
    pos: egui::Pos2,
    text: &str,
    font_id: egui::FontId,
    base_color: egui::Color32,
    is_tribe: bool,
) {
    if text.is_empty() {
        return;
    }

    // Opaque Matte Black for outline and 3D dragged shadow (Supercell style!)
    let black = egui::Color32::BLACK;

    if is_tribe {
        // Optimized zero-cost pristine 2-pass shadow for Tribes!
        let shadow_pos = pos + egui::vec2(1.0, 1.0);
        painter.text(
            shadow_pos,
            egui::Align2::LEFT_TOP,
            text,
            font_id.clone(),
            black,
        );
        painter.text(pos, egui::Align2::LEFT_TOP, text, font_id, base_color);
        return;
    }

    // 1. Optimized dragged-down 3D Opaque Black Shadow (2 passes instead of 12!)
    for &dy in &[2.0, 4.0] {
        painter.text(
            pos + egui::vec2(0.0, dy),
            egui::Align2::LEFT_TOP,
            text,
            font_id.clone(),
            black,
        );
    }

    // 2. Optimized 4-way diagonal outline (4 passes instead of 8!)
    for &(dx, dy) in &[(-1.5, -1.5), (1.5, -1.5), (-1.5, 1.5), (1.5, 1.5)] {
        painter.text(
            pos + egui::vec2(dx, dy),
            egui::Align2::LEFT_TOP,
            text,
            font_id.clone(),
            black,
        );
    }

    // 3. Core text (1 pass)
    painter.text(pos, egui::Align2::LEFT_TOP, text, font_id, base_color);
}

pub fn paint_glow_nameplate_galley(
    painter: &egui::Painter,
    pos: egui::Pos2,
    galley: Arc<egui::Galley>,
    base_color: egui::Color32,
    font_id: egui::FontId,
    is_tribe: bool,
) {
    if !galley.is_empty() {
        paint_glow_text(painter, pos, galley.text(), font_id, base_color, is_tribe);
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
