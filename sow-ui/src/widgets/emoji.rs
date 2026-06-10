use egui::{Align2, Color32, CursorIcon, FontId, Pos2, Rect, Response, Sense, Ui, Vec2, Widget};

enum Run<'a> {
    Text(&'a str),
    Emoji(&'a str),
}

fn match_emoji_at(text: &str, byte_idx: usize) -> Option<usize> {
    let rest = &text[byte_idx..];
    let mut best = None;
    for (ci, (_, _)) in rest.char_indices().enumerate() {
        if ci > 8 {
            break;
        }
        let end = rest
            .char_indices()
            .nth(ci + 1)
            .map(|(j, _)| j)
            .unwrap_or(rest.len());
        let candidate = &rest[..end];
        if sow_core::emoji::atlas_uv(candidate).is_some() {
            best = Some(byte_idx + end);
        }
    }
    best
}

fn split_runs(text: &str) -> Vec<Run<'_>> {
    let mut runs = Vec::new();
    let mut i = 0;
    while i < text.len() {
        if let Some(end) = match_emoji_at(text, i) {
            runs.push(Run::Emoji(&text[i..end]));
            i = end;
        } else {
            let start = i;
            i += text[i..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
            while i < text.len() {
                if match_emoji_at(text, i).is_some() {
                    break;
                }
                i += text[i..]
                    .chars()
                    .next()
                    .map(|c| c.len_utf8())
                    .unwrap_or(1);
            }
            runs.push(Run::Text(&text[start..i]));
        }
    }
    runs
}

fn emoji_icon_size(font: &FontId) -> f32 {
    font.size * 1.4
}

fn measure_runs(painter: &egui::Painter, runs: &[Run<'_>], font: &FontId) -> (f32, f32) {
    let icon_h = emoji_icon_size(font);
    let mut w = 0.0_f32;
    let mut h = font.size;
    for run in runs {
        match run {
            Run::Text(s) => {
                let g = painter.layout_no_wrap((*s).to_owned(), font.clone(), Color32::WHITE);
                w += g.rect.width();
                h = h.max(g.rect.height());
            }
            Run::Emoji(_) => {
                w += icon_h + 2.0;
                h = h.max(icon_h);
            }
        }
    }
    (w, h)
}

/// Draw a pixel emoji from the embedded atlas. Returns false if the glyph is not in the atlas.
pub fn try_paint_emoji(painter: &egui::Painter, emoji: &str, rect: Rect, tint: Color32) -> bool {
    let Some(uv) = sow_core::emoji::atlas_uv(emoji) else {
        return false;
    };
    let Some(texture) = sow_core::emoji::atlas_texture(painter.ctx()) else {
        return false;
    };

    let alpha = (tint.a() as f32 * 0.65) as u8;
    if alpha > 0 {
        let shadow_tint = Color32::from_black_alpha(alpha);
        // Premium 4-way diagonal outline + 2 dragged shadows matching the text glow style perfectly
        let offsets = [
            (-1.2, -1.2),
            (1.2, -1.2),
            (-1.2, 1.2),
            (1.2, 1.2),
            (0.0, 1.5),
            (0.0, 3.0),
        ];
        for (dx, dy) in offsets {
            painter.image(
                texture.id(),
                rect.translate(egui::vec2(dx, dy)),
                uv,
                shadow_tint,
            );
        }
    }

    // Only use the alpha component of tint to preserve original emoji colors while allowing transparency
    let final_tint = Color32::from_white_alpha(tint.a());
    painter.image(texture.id(), rect, uv, final_tint);
    true
}

pub fn paint_emoji_centered(
    painter: &egui::Painter,
    emoji: &str,
    center: Pos2,
    size: f32,
    tint: Color32,
) -> bool {
    let rect = Rect::from_center_size(center, egui::vec2(size, size));
    try_paint_emoji(painter, emoji, rect, tint)
}

/// Inline label mixing atlas emoji and normal text.
pub fn emoji_label(ui: &mut Ui, text: &str, font_id: FontId, color: Color32) -> Response {
    let runs = split_runs(text);
    let (total_w, max_h) = measure_runs(ui.painter(), &runs, &font_id);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(total_w, max_h), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_runs(ui.painter(), rect, &runs, &font_id, color, false);
    }
    response
}

/// Outlined inline label (text parts get glow; emoji painted from atlas).
pub fn outlined_emoji_label(ui: &mut Ui, text: &str, font_id: FontId, color: Color32) -> Response {
    let runs = split_runs(text);
    let (total_w, max_h) = measure_runs(ui.painter(), &runs, &font_id);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(total_w, max_h), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_runs(ui.painter(), rect, &runs, &font_id, color, true);
    }
    response
}

/// Outlined inline text at a painter anchor (for floating HUD text).
pub fn outlined_emoji_text(
    painter: &egui::Painter,
    pos: Pos2,
    anchor: Align2,
    text: &str,
    font_id: FontId,
    color: Color32,
    shadow: Color32,
) {
    paint_emoji_text_at(painter, pos, anchor, text, font_id, color, true);
    let _ = shadow;
}

pub fn measure_emoji_text(painter: &egui::Painter, text: &str, font_id: &FontId) -> Vec2 {
    let runs = split_runs(text);
    let (w, h) = measure_runs(painter, &runs, font_id);
    Vec2::new(w, h)
}

pub fn paint_emoji_text_at(
    painter: &egui::Painter,
    pos: Pos2,
    anchor: Align2,
    text: &str,
    font_id: FontId,
    color: Color32,
    outlined: bool,
) {
    let runs = split_runs(text);
    let (total_w, max_h) = measure_runs(painter, &runs, &font_id);
    let rect = anchor.anchor_size(pos, Vec2::new(total_w, max_h));
    paint_runs(painter, rect, &runs, &font_id, color, outlined);
}

fn paint_runs(
    painter: &egui::Painter,
    rect: Rect,
    runs: &[Run<'_>],
    font_id: &FontId,
    color: Color32,
    outlined: bool,
) {
    let icon_h = emoji_icon_size(font_id);
    let mut x = rect.left();
    let cy = rect.center().y;
    for run in runs {
        match run {
            Run::Text(s) => {
                let galley = painter.layout_no_wrap(s.to_string(), font_id.clone(), color);
                let w = galley.rect.width();
                if outlined {
                    let shadow_color = Color32::from_black_alpha(color.a());
                    crate::ui::theme::outlined_text(
                        painter,
                        Pos2::new(x, cy),
                        Align2::LEFT_CENTER,
                        s,
                        font_id.clone(),
                        color,
                        shadow_color,
                    );
                } else {
                    let y = cy - galley.rect.height() / 2.0;
                    painter.galley(Pos2::new(x, y), galley, color);
                }
                x += w;
            }
            Run::Emoji(e) => {
                let r = Rect::from_center_size(Pos2::new(x + icon_h / 2.0, cy), Vec2::splat(icon_h));
                try_paint_emoji(painter, e, r, color);
                x += icon_h + 2.0;
            }
        }
    }
}

/// Clickable HUD square that renders a pixel emoji (falls back to glow text if missing).
pub struct HudEmojiButton {
    emoji: String,
    color: Color32,
    dim: Option<f32>,
}

impl HudEmojiButton {
    pub fn new(emoji: impl Into<String>) -> Self {
        Self {
            emoji: emoji.into(),
            color: Color32::WHITE,
            dim: None,
        }
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    pub fn dim(mut self, dim: f32) -> Self {
        self.dim = Some(dim);
        self
    }
}

impl Widget for HudEmojiButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let size = self.dim.unwrap_or_else(|| {
            if cfg!(target_os = "android") {
                48.0
            } else {
                32.0
            }
        });
        let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), Sense::click());

        let is_hovered = response.hovered();
        let is_active = response.is_pointer_button_down_on();
        let hover_t = ui.ctx().animate_bool(response.id.with("hover"), is_hovered);
        let active_t = ui.ctx().animate_bool(response.id.with("active"), is_active);
        let alpha = (hover_t * 25.0 + active_t * 25.0) as u8;
        if alpha > 0 {
            ui.painter()
                .rect_filled(rect, 6.0, Color32::from_white_alpha(alpha));
        }

        let emoji_size = size * 0.88;
        if !try_paint_emoji(
            ui.painter(),
            &self.emoji,
            Rect::from_center_size(rect.center(), egui::vec2(emoji_size, emoji_size)),
            self.color,
        ) {
            let font_id = egui::FontId::proportional(crate::ui::theme::hud_button_text_size());
            crate::ui::theme::paint_premium_glow_text(
                ui.painter(),
                rect.center(),
                Align2::CENTER_CENTER,
                &self.emoji,
                font_id,
                self.color,
                Color32::BLACK,
            );
        }

        response.on_hover_cursor(CursorIcon::PointingHand)
    }
}
