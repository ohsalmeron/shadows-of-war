use egui::{Color32, CornerRadius, Response, Stroke, Ui, Widget};
use sow_core::protocol::LobbyInfo;

pub struct LobbyCard<'a> {
    lobby: &'a LobbyInfo,
    texture: Option<&'a egui::TextureHandle>,
    side: Option<f32>,
    width: Option<f32>,
    /// Display name override (e.g. catalog display name); falls back to map_name.
    display_name: Option<String>,
}

impl<'a> LobbyCard<'a> {
    pub fn new(lobby: &'a LobbyInfo, texture: Option<&'a egui::TextureHandle>) -> Self {
        Self {
            lobby,
            texture,
            side: None,
            width: None,
            display_name: None,
        }
    }

    /// Set height constraint for the card.
    pub fn side(mut self, side: f32) -> Self {
        self.side = Some(side);
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Pretty map name (catalog display name); defaults to the raw slug.
    pub fn display_name(mut self, display_name: String) -> Self {
        self.display_name = Some(display_name);
        self
    }
}

pub struct LobbyCardResponse {
    pub clicked: bool,
}

impl<'a> Widget for LobbyCard<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let aspect_ratio = 16.0 / 9.0;
        let max_w = self.width.unwrap_or_else(|| ui.available_width());
        let mut w = max_w;
        let mut h = w / aspect_ratio;

        if let Some(limit_h) = self.side {
            if h > limit_h {
                h = limit_h;
                w = h * aspect_ratio;
            }
        }

        let desired_size = egui::vec2(w, h);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

        let is_hovered = response.hovered();
        if is_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let time = ui.input(|i| i.time);
        let pulse = (((time * 3.5).sin() + 1.0) * 0.5) as f32; // smooth 0.0 to 1.0 pulse

        let is_matchmaking = self.lobby.kind == sow_core::protocol::LobbyKind::Matchmaking;

        let mut stroke_color = if self.lobby.is_counting_down {
            if is_hovered {
                sow_ui_kit::theme::palette::neon_cyan_hover()
            } else {
                sow_ui_kit::theme::palette::neon_cyan()
            }
        } else {
            if is_hovered {
                sow_ui_kit::theme::palette::pink()
            } else {
                sow_ui_kit::theme::palette::field_border()
            }
        };

        if is_matchmaking && !self.lobby.is_counting_down {
            let start_c = sow_ui_kit::theme::palette::field_border();
            let end_c = sow_ui_kit::theme::palette::neon_cyan();
            stroke_color = Color32::from_rgb(
                (start_c.r() as f32 + (end_c.r() as f32 - start_c.r() as f32) * pulse) as u8,
                (start_c.g() as f32 + (end_c.g() as f32 - start_c.g() as f32) * pulse) as u8,
                (start_c.b() as f32 + (end_c.b() as f32 - start_c.b() as f32) * pulse) as u8,
            );
            ui.ctx().request_repaint(); // keep animating
        }

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
                egui::CornerRadius::same(12),
            );
        } else {
            ui.painter()
                .rect_filled(rect, 12.0, sow_ui_kit::theme::palette::button_inactive());
        }

        let top_rect = rect.shrink(8.0);
        let mode_text = if self.lobby.game_mode == "FFA" {
            "FFA"
        } else {
            "TEAMS"
        };

        paint_badge(
            ui.painter(),
            (top_rect.min, false),
            (mode_text, egui::FontId::proportional(14.0)),
            (Color32::WHITE, sow_ui_kit::theme::palette::neon_cyan(), false),
        );

        let timer_text = if self.lobby.is_counting_down {
            format!("Starts in {:.0}s", self.lobby.timer_secs.max(0.0))
        } else if is_matchmaking {
            "SEARCHING".to_string()
        } else {
            "WAITING".to_string()
        };
        let timer_color = if self.lobby.is_counting_down {
            Color32::from_rgb(255, 210, 120)
        } else if is_matchmaking {
            let start_val = 140.0;
            let end_r = sow_ui_kit::theme::palette::neon_cyan().r() as f32;
            let end_g = sow_ui_kit::theme::palette::neon_cyan().g() as f32;
            let end_b = sow_ui_kit::theme::palette::neon_cyan().b() as f32;
            Color32::from_rgb(
                (start_val + (end_r - start_val) * pulse) as u8,
                (start_val + (end_g - start_val) * pulse) as u8,
                (start_val + (end_b - start_val) * pulse) as u8,
            )
        } else {
            sow_ui_kit::theme::palette::text_muted()
        };
        paint_badge(
            ui.painter(),
            (egui::pos2(top_rect.max.x, top_rect.min.y), true),
            (&timer_text, sow_ui_kit::theme::font_regular(14.0)),
            (timer_color, Color32::from_black_alpha(180), true),
        );

        let bottom_height = 62.0;
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
            Color32::from_black_alpha(210),
        );

        let pad = 10.0;

        // "GO" button at the bottom-right of the panel. The whole card stays the click target —
        // this just sells the affordance. Reserve its column so the left text stops before it.
        let btn_w = 64.0_f32.min((rect.width() * 0.32).max(48.0));
        let btn_h = 42.0_f32;
        let btn_rect = egui::Rect::from_min_max(
            egui::pos2(
                bottom_rect.max.x - pad - btn_w,
                bottom_rect.center().y - btn_h * 0.5,
            ),
            egui::pos2(
                bottom_rect.max.x - pad,
                bottom_rect.center().y + btn_h * 0.5,
            ),
        );
        let text_right = btn_rect.min.x - pad;

        let line1_y = bottom_rect.min.y + 8.0;
        let line2_y = line1_y + 20.0;

        // Players badge — pulled OUT of the panel, floating just above its top-right corner.
        let players_text = format!(
            "{}/{} players",
            self.lobby.num_players, self.lobby.max_players
        );
        paint_badge(
            ui.painter(),
            (egui::pos2(rect.max.x - pad, bottom_rect.min.y - 26.0), true),
            (&players_text, egui::FontId::proportional(12.0)),
            (Color32::WHITE, Color32::from_black_alpha(200), false),
        );

        // Line 1: map name (left).
        let map_text = self
            .display_name
            .as_deref()
            .unwrap_or(&self.lobby.map_name)
            .to_uppercase();
        sow_ui_kit::theme::paint_premium_glow_text(
            ui.painter(),
            egui::pos2(bottom_rect.min.x + pad, line1_y),
            egui::Align2::LEFT_TOP,
            &map_text,
            egui::FontId::proportional(15.0),
            Color32::WHITE,
            Color32::BLACK,
        );

        // Line 2: bot/nation counts + difficulty (left) + host (right, left of the button).
        let diff_str = match self.lobby.bot_difficulty {
            sow_core::game_config::BotDifficulty::Terminator => "Terminator",
            _ => "Vanilla",
        };
        let stats_text = format!(
            "{} tribes  {}  nations  {}",
            self.lobby.bot_count, self.lobby.nation_count, diff_str,
        );
        ui.painter().text(
            egui::pos2(bottom_rect.min.x + pad, line2_y),
            egui::Align2::LEFT_TOP,
            &stats_text,
            egui::FontId::proportional(11.0),
            sow_ui_kit::theme::palette::text_muted(),
        );

        if !self.lobby.host_name.is_empty() {
            let host_text = format!("by {}", self.lobby.host_name);
            ui.painter().text(
                egui::pos2(text_right, line2_y),
                egui::Align2::RIGHT_TOP,
                &host_text,
                egui::FontId::proportional(11.0),
                sow_ui_kit::theme::palette::neon_cyan(),
            );
        }

        ui.painter().rect_stroke(
            rect,
            12.0,
            Stroke::new(stroke_width, stroke_color),
            egui::StrokeKind::Inside,
        );

        // The "GO" button on top, lit by the card's own hover state.
        let hot = ui
            .ctx()
            .animate_bool(response.id.with("lobby_go_hot"), is_hovered);
        if is_hovered {
            ui.ctx().request_repaint(); // breathe while hovered
        }
        crate::widgets::paint_play_button(ui.painter(), btn_rect, hot, pulse, "GO");

        response
    }
}

fn paint_badge(
    painter: &egui::Painter,
    placement: (egui::Pos2, bool),
    label: (&str, egui::FontId),
    style: (Color32, Color32, bool),
) {
    let (pos, align_right) = placement;
    let (text, font_id) = label;
    let (text_color, bg_color, glow) = style;
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
        sow_ui_kit::theme::paint_premium_glow_text(
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
