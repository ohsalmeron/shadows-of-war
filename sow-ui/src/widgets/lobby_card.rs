use egui::{Color32, CornerRadius, Response, Stroke, Ui, Widget};
use sow_core::protocol::LobbyInfo;

pub struct LobbyCard<'a> {
    lobby: &'a LobbyInfo,
    texture: Option<&'a egui::TextureHandle>,
    max_h: f32,
}

impl<'a> LobbyCard<'a> {
    pub fn new(lobby: &'a LobbyInfo, texture: Option<&'a egui::TextureHandle>) -> Self {
        Self {
            lobby,
            texture,
            max_h: 160.0,
        }
    }

    pub fn max_h(mut self, max_h: f32) -> Self {
        self.max_h = max_h;
        self
    }
}

pub struct LobbyCardResponse {
    pub clicked: bool,
}

impl<'a> Widget for LobbyCard<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let max_w = ui.available_width();
        let mut w = max_w;
        let mut h = self.max_h;

        if let Some(texture) = self.texture {
            let tex_size = texture.size();
            let tex_aspect = tex_size[0] as f32 / tex_size[1] as f32;
            let box_aspect = max_w / self.max_h;

            if tex_aspect > box_aspect {
                w = max_w;
                h = max_w / tex_aspect;
            } else {
                h = self.max_h;
                w = self.max_h * tex_aspect;
            }
        }

        let desired_size = egui::vec2(w, h);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

        let is_hovered = response.hovered();
        if is_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let stroke_color = if self.lobby.is_counting_down {
            if is_hovered {
                crate::ui::theme::accent_solo_cyan_hover()
            } else {
                crate::ui::theme::accent_solo_cyan()
            }
        } else {
            if is_hovered {
                crate::ui::theme::avatar_pink()
            } else {
                crate::ui::theme::nickname_field_border()
            }
        };

        let stroke_width = if is_hovered { 2.0_f32 } else { 1.0_f32 };

        if let Some(texture) = self.texture {
            let tint = if is_hovered {
                Color32::WHITE
            } else {
                Color32::from_gray(200)
            };
            let image = egui::Image::new(texture)
                .fit_to_exact_size(rect.size())
                .corner_radius(CornerRadius::same(12))
                .tint(tint);
            ui.put(rect, image);
        } else {
            ui.painter()
                .rect_filled(rect, 12.0, crate::ui::theme::menu_secondary_button());
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

        let mode_galley = ui.painter().layout_no_wrap(
            mode_text.to_string(),
            egui::FontId::proportional(14.0),
            Color32::WHITE,
        );
        let mode_badge_rect =
            egui::Rect::from_min_size(top_rect.min, mode_galley.size() + egui::vec2(12.0, 6.0));
        ui.painter()
            .rect_filled(mode_badge_rect, 4.0, crate::ui::theme::accent_solo_cyan());
        ui.painter().galley(
            mode_badge_rect.center() - mode_galley.size() / 2.0,
            mode_galley,
            Color32::WHITE,
        );

        let timer_text = if self.lobby.is_counting_down {
            format!("Starts in {:.0}s", self.lobby.timer_secs.max(0.0))
        } else {
            "WAITING".to_string()
        };
        let timer_color = if self.lobby.is_counting_down {
            Color32::from_rgb(255, 210, 120)
        } else {
            crate::ui::theme::text_secondary()
        };
        let timer_text_str = timer_text.clone();
        let timer_galley =
            ui.painter()
                .layout_no_wrap(timer_text, egui::FontId::proportional(14.0), timer_color);
        let timer_badge_rect = egui::Rect::from_min_size(
            egui::pos2(
                top_rect.max.x - timer_galley.size().x - 12.0,
                top_rect.min.y,
            ),
            timer_galley.size() + egui::vec2(12.0, 6.0),
        );
        ui.painter()
            .rect_filled(timer_badge_rect, 4.0, Color32::from_black_alpha(180));
        crate::ui::theme::outlined_text(
            ui.painter(),
            timer_badge_rect.center() - timer_galley.size() / 2.0,
            egui::Align2::LEFT_TOP,
            &timer_text_str,
            egui::FontId::proportional(14.0),
            timer_color,
            Color32::BLACK,
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
        crate::ui::theme::outlined_text(
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
        let players_text_str = players_text.clone();
        let players_galley = ui.painter().layout_no_wrap(
            players_text,
            egui::FontId::proportional(14.0),
            Color32::WHITE,
        );
        let players_badge_rect = egui::Rect::from_min_size(
            egui::pos2(
                bottom_rect.max.x - players_galley.size().x - 16.0,
                bottom_rect.min.y - 12.0,
            ),
            players_galley.size() + egui::vec2(12.0, 6.0),
        );
        ui.painter()
            .rect_filled(players_badge_rect, 4.0, Color32::from_black_alpha(220));
        crate::ui::theme::outlined_text(
            ui.painter(),
            players_badge_rect.center() - players_galley.size() / 2.0,
            egui::Align2::LEFT_TOP,
            &players_text_str,
            egui::FontId::proportional(14.0),
            Color32::WHITE,
            Color32::BLACK,
        );

        response
    }
}
