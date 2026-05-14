use std::collections::HashMap;
use std::sync::Arc;
use egui::text::{LayoutJob, TextFormat};

pub struct CachedNameplate {
    pub name_galley: Arc<egui::Galley>,
    pub troops_galley: Arc<egui::Galley>,
    pub last_formatted_troops: String,
    pub last_font_size: f32,
}

/// Nameplate troop text: snap to sim at most ~2/s per player (OpenFront-style).
#[derive(Default)]
pub struct TroopLabelThrottle {
    last_refresh_wall_secs: HashMap<u16, f64>,
    shown_troops: HashMap<u16, f64>,
}

impl TroopLabelThrottle {
    pub const INTERVAL: f64 = 0.5;

    pub fn displayed_troops(&mut self, wall_secs: f64, player_id: u16, sim_troops: f64) -> f64 {
        let refresh = match self.last_refresh_wall_secs.get(&player_id) {
            None => true,
            Some(&t) if wall_secs - t >= Self::INTERVAL => true,
            _ => false,
        };
        if refresh {
            self.last_refresh_wall_secs.insert(player_id, wall_secs);
            self.shown_troops.insert(player_id, sim_troops);
            sim_troops
        } else {
            *self.shown_troops.get(&player_id).unwrap_or(&sim_troops)
        }
    }

    pub fn clear(&mut self) {
        self.last_refresh_wall_secs.clear();
        self.shown_troops.clear();
    }
}

#[derive(Default)]
pub struct NameBoxThrottle {
    pub cached_boxes: HashMap<u16, crate::name_box::NameBox>,
    last_tile_count: HashMap<u16, u32>,
    last_refresh_wall_secs: HashMap<u16, f64>,
}

impl NameBoxThrottle {
    pub const INTERVAL: f64 = 1.0; // Recalculate at most once per second per player
    
    pub fn update_and_get(
        &mut self,
        wall_secs: f64,
        player_id: u16,
        tile_count: u32,
        map_w: u32,
        map_h: u32,
        owners: &[u16],
        terrain: &[u8],
    ) -> Option<crate::name_box::NameBox> {
        let last_count = *self.last_tile_count.get(&player_id).unwrap_or(&0);
        let last_time = *self.last_refresh_wall_secs.get(&player_id).unwrap_or(&0.0);
        
        let needs_update = last_count != tile_count && (wall_secs - last_time >= Self::INTERVAL);
        
        if needs_update || !self.cached_boxes.contains_key(&player_id) {
            self.last_refresh_wall_secs.insert(player_id, wall_secs);
            self.last_tile_count.insert(player_id, tile_count);
            
            if let Some(name_box) = crate::name_box::calculate_name_box(player_id, map_w, map_h, owners, terrain) {
                self.cached_boxes.insert(player_id, name_box);
            }
        }
        
        self.cached_boxes.get(&player_id).copied()
    }
    
    pub fn clear(&mut self) {
        self.cached_boxes.clear();
        self.last_tile_count.clear();
        self.last_refresh_wall_secs.clear();
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
pub fn paint_nameplate_galley(
    painter: &egui::Painter,
    pos: egui::Pos2,
    galley: Arc<egui::Galley>,
) {
    if !galley.is_empty() {
        painter.galley(pos, galley, NAMEPLATE_FILL);
    }
}

pub fn layout_nameplate_name_galley(
    painter: &egui::Painter,
    font_id: egui::FontId,
    name: &str,
    is_human: bool,
    player_color: egui::Color32,
) -> Arc<egui::Galley> {
    if is_human {
        let mut job = LayoutJob { break_on_newline: false, ..Default::default() };
        job.append(
            "★ ",
            0.0,
            TextFormat::simple(font_id.clone(), player_color),
        );
        job.append(name, 0.0, TextFormat::simple(font_id, NAMEPLATE_FILL));
        painter.layout_job(job)
    } else {
        painter.layout_no_wrap(name.to_owned(), font_id, NAMEPLATE_FILL)
    }
}

pub fn layout_nameplate_troops_galley(
    painter: &egui::Painter,
    font_id: egui::FontId,
    troops_str: &str,
) -> Arc<egui::Galley> {
    let mut job = LayoutJob { break_on_newline: false, ..Default::default() };
    job.append(
        "⚔ ",
        0.0,
        TextFormat::simple(font_id.clone(), egui::Color32::BLACK),
    );
    job.append(troops_str, 0.0, TextFormat::simple(font_id, NAMEPLATE_FILL));
    painter.layout_job(job)
}

pub fn render_troops(mut num: f64) -> String {
    num = num.max(0.0);
    if num >= 10_000_000.0 {
        let value = (num / 100_000.0).floor() / 10.0;
        format!("{:.1}M", value)
    } else if num >= 1_000_000.0 {
        let value = (num / 10_000.0).floor() / 100.0;
        format!("{:.2}M", value)
    } else if num >= 100_000.0 {
        format!("{}K", (num / 1000.0).floor())
    } else if num >= 10_000.0 {
        let value = (num / 100.0).floor() / 10.0;
        format!("{:.1}K", value)
    } else if num >= 1_000.0 {
        let value = (num / 10.0).floor() / 100.0;
        format!("{:.2}K", value)
    } else {
        format!("{:.0}", num.floor())
    }
}
