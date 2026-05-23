use std::collections::HashMap;
use std::sync::Arc;



/// Nameplate troop text: matches with each tick for 100% sync.
#[derive(Default)]
pub struct TroopLabelThrottle {
    shown_troops: HashMap<u16, f64>,
}

impl TroopLabelThrottle {
    pub fn displayed_troops(&mut self, _tick: u64, player_id: u16, sim_troops: f64) -> f64 {
        self.shown_troops.insert(player_id, sim_troops);
        sim_troops
    }

    pub fn clear(&mut self) {
        self.shown_troops.clear();
    }
}

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
    rect_size: egui::Vec2,
) {
    if text.is_empty() {
        return;
    }

    // Opaque Matte Black for outline and 3D dragged shadow (Supercell style!)
    let black = egui::Color32::BLACK;

    // 1. Dragged-down 3D Opaque Black Shadow (creates a solid 3D block)
    for dy in &[1.0, 2.0, 3.0, 4.0] {
        painter.text(pos + egui::vec2(-1.5, *dy), egui::Align2::LEFT_TOP, text, font_id.clone(), black);
        painter.text(pos + egui::vec2(1.5, *dy), egui::Align2::LEFT_TOP, text, font_id.clone(), black);
        painter.text(pos + egui::vec2(0.0, *dy), egui::Align2::LEFT_TOP, text, font_id.clone(), black);
    }

    // 2. Thick Opaque Black Outline (8-way 1.5px offset for bold strategic style)
    for dx in &[-1.5, 0.0, 1.5] {
        for dy in &[-1.5, 0.0, 1.5] {
            if *dx != 0.0 || *dy != 0.0 {
                painter.text(pos + egui::vec2(*dx, *dy), egui::Align2::LEFT_TOP, text, font_id.clone(), black);
            }
        }
    }

    // 3. Top-to-Bottom Gradient Core: top is the pure base color (brightest), bottom is 50% brightness of the base color
    let bright_top = base_color;

    // Dark bottom: 50% brightness of the base color
    let dark_bottom = egui::Color32::from_rgb(
        (base_color.r() as u32 * 50 / 100) as u8,
        (base_color.g() as u32 * 50 / 100) as u8,
        (base_color.b() as u32 * 50 / 100) as u8,
    );

    // Draw the bright top layer first
    painter.text(pos, egui::Align2::LEFT_TOP, text, font_id.clone(), bright_top);

    // Draw the dark bottom layer clipped to the bottom half of the text rect
    let text_rect = egui::Rect::from_min_size(pos, rect_size);
    let bottom_clip = egui::Rect::from_min_max(
        egui::pos2(text_rect.min.x - 10.0, text_rect.center().y),
        egui::pos2(text_rect.max.x + 10.0, text_rect.max.y + 10.0),
    );
    let clipped_painter = painter.with_clip_rect(bottom_clip);
    clipped_painter.text(pos, egui::Align2::LEFT_TOP, text, font_id, dark_bottom);
}

pub fn paint_glow_nameplate_galley(
    painter: &egui::Painter,
    pos: egui::Pos2,
    galley: Arc<egui::Galley>,
    base_color: egui::Color32,
    font_id: egui::FontId,
) {
    if !galley.is_empty() {
        paint_glow_text(painter, pos, &galley.text(), font_id, base_color, galley.rect.size());
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
    painter.layout_no_wrap(troops_str.to_owned(), font_id, NAMEPLATE_FILL)
}
