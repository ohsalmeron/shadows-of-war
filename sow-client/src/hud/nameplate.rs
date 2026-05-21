use egui::text::{LayoutJob, TextFormat};
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
    let mut job = LayoutJob {
        break_on_newline: false,
        ..Default::default()
    };
    job.append(
        "⚔ ",
        0.0,
        TextFormat::simple(font_id.clone(), egui::Color32::BLACK),
    );
    job.append(troops_str, 0.0, TextFormat::simple(font_id, NAMEPLATE_FILL));
    painter.layout_job(job)
}
