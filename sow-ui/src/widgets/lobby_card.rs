use egui::{Color32, CornerRadius, Response, Stroke, Ui, Widget};
use sow_core::protocol::LobbyInfo;

pub struct LobbyCard<'a> {
    lobby: &'a LobbyInfo,
    texture: Option<&'a egui::TextureHandle>,
    side: f32,
}

impl<'a> LobbyCard<'a> {
    pub fn new(lobby: &'a LobbyInfo, texture: Option<&'a egui::TextureHandle>) -> Self {
        Self {
            lobby,
            texture,
            side: 160.0,
        }
    }

    /// Square edge length (1:1 — standard for map thumbnails).
    pub fn side(mut self, side: f32) -> Self {
        self.side = side;
        self
    }
}

pub struct LobbyCardResponse {
    pub clicked: bool,
}

impl<'a> Widget for LobbyCard<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let w = ui.available_width();
        let h = self.side;
        let desired_size = egui::vec2(w, h);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

        let is_hovered = response.hovered();
        if is_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let stroke_color = if self.lobby.is_counting_down {
            if is_hovered {
                crate::ui::theme::palette::neon_cyan_hover()
            } else {
                crate::ui::theme::palette::neon_cyan()
            }
        } else {
            if is_hovered {
                crate::ui::theme::palette::pink()
            } else {
                crate::ui::theme::palette::field_border()
            }
        };

        let stroke_width = if is_hovered { 2.0_f32 } else { 1.0_f32 };

        if let Some(texture) = self.texture {
            let brightness = if is_hovered { 1.2 } else { 1.0 };
            let uv = crate::ui::map_texture::cover_uv(rect.size(), texture.size_vec2());
            crate::ui::map_texture::draw_map_thumbnail_uv(
                ui.painter(),
                texture.id(),
                rect,
                uv,
                brightness,
            );
        } else {
            ui.painter()
                .rect_filled(rect, 12.0, crate::ui::theme::palette::button_inactive());
        }

        ui.painter().rect_stroke(
            rect,
            12.0,
            Stroke::new(stroke_width, stroke_color),
            egui::StrokeKind::Inside,
        );

        let top_rect = rect.shrink(8.0);
        let mode_text = if self.lobby.game_mode == "FFA" {
            "FFA"
        } else {
            "TEAMS"
        };

        paint_badge(
            ui.painter(),
            top_rect.min,
            false,
            mode_text,
            egui::FontId::proportional(14.0),
            Color32::WHITE,
            crate::ui::theme::palette::neon_cyan(),
            false,
        );

        let timer_text = if self.lobby.is_counting_down {
            format!("Starts in {:.0}s", self.lobby.timer_secs.max(0.0))
        } else {
            "WAITING".to_string()
        };
        let timer_color = if self.lobby.is_counting_down {
            Color32::from_rgb(255, 210, 120)
        } else {
            crate::ui::theme::palette::text_muted()
        };
        paint_badge(
            ui.painter(),
            egui::pos2(top_rect.max.x, top_rect.min.y),
            true,
            &timer_text,
            crate::ui::theme::font_regular(14.0),
            timer_color,
            Color32::from_black_alpha(180),
            true,
        );

        let bottom_height = 44.0;
        let bottom_rect =
            egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.max.y - bottom_height), rect.max);
        ui.painter().rect_filled(
            bottom_rect,
            CornerRadius {
                nw: 0,
                ne: 0,
                sw: 12,
                se: 12,
            },
            Color32::from_black_alpha(200),
        );

        let map_text = self.lobby.map_name.to_uppercase();
        let map_galley = ui.painter().layout_no_wrap(
            map_text.clone(),
            egui::FontId::proportional(18.0),
            Color32::WHITE,
        );
        crate::ui::theme::paint_premium_glow_text(
            ui.painter(),
            egui::pos2(
                bottom_rect.min.x + 12.0,
                bottom_rect.min.y + (bottom_height - map_galley.size().y) / 2.0,
            ),
            egui::Align2::LEFT_TOP,
            &map_text,
            egui::FontId::proportional(18.0),
            Color32::WHITE,
            Color32::BLACK,
        );

        let players_text = format!("{}/{}", self.lobby.num_players, self.lobby.max_players);
        paint_badge(
            ui.painter(),
            egui::pos2(bottom_rect.max.x - 4.0, bottom_rect.min.y - 12.0),
            true,
            &players_text,
            egui::FontId::proportional(14.0),
            Color32::WHITE,
            Color32::from_black_alpha(220),
            true,
        );

        response
    }
}

fn paint_badge(
    painter: &egui::Painter,
    pos: egui::Pos2,
    align_right: bool,
    text: &str,
    font_id: egui::FontId,
    text_color: Color32,
    bg_color: Color32,
    glow: bool,
) {
    let galley = painter.layout_no_wrap(text.to_string(), font_id.clone(), text_color);
    let size = galley.size() + egui::vec2(12.0, 6.0);
    let min = if align_right {
        egui::pos2(pos.x - size.x, pos.y)
    } else {
        pos
    };
    let badge_rect = egui::Rect::from_min_size(min, size);
    painter.rect_filled(badge_rect, 4.0, bg_color);
    let text_pos = badge_rect.center() - galley.size() / 2.0;
    if glow {
        crate::ui::theme::paint_premium_glow_text(
            painter,
            text_pos,
            egui::Align2::LEFT_TOP,
            text,
            font_id,
            text_color,
            Color32::BLACK,
        );
    } else {
        painter.galley(text_pos, galley, text_color);
    }
}
