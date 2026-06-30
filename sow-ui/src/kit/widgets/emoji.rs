use std::sync::Arc;

use egui::{
    Align2, Color32, CursorIcon, FontId, Galley, Pos2, Rect, Response, Sense, Ui, Vec2, Widget,
};

enum Run<'a> {
    Text(&'a str),
    Emoji(&'a str),
}

#[derive(Clone)]
pub enum PreparedRun {
    Text {
        galley: Arc<Galley>,
        width: f32,
        height: f32,
    },
    Emoji {
        emoji: String,
        width: f32,
        height: f32,
    },
}

#[derive(Clone)]
pub struct PreparedName {
    pub size: Vec2,
    runs: Vec<PreparedRun>,
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
        if crate::atlas_uv(candidate).is_some() {
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
            i += text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            while i < text.len() {
                if match_emoji_at(text, i).is_some() {
                    break;
                }
                i += text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            }
            runs.push(Run::Text(&text[start..i]));
        }
    }
    runs
}

fn emoji_icon_size(font: &FontId) -> f32 {
    font.size * 1.4
}

/// Parse and layout a mixed emoji/text string once for reuse across frames.
pub fn prepare_name(painter: &egui::Painter, text: &str, font_id: &FontId) -> PreparedName {
    let runs = split_runs(text);
    let icon_h = emoji_icon_size(font_id);
    let mut prepared_runs = Vec::with_capacity(runs.len());
    let mut w = 0.0_f32;
    let mut h = font_id.size;

    for run in runs {
        match run {
            Run::Text(s) => {
                let galley = painter.layout_no_wrap(s.to_owned(), font_id.clone(), Color32::WHITE);
                let rw = galley.rect.width();
                let rh = galley.rect.height();
                w += rw;
                h = h.max(rh);
                prepared_runs.push(PreparedRun::Text {
                    galley,
                    width: rw,
                    height: rh,
                });
            }
            Run::Emoji(e) => {
                let rw = icon_h + 2.0;
                prepared_runs.push(PreparedRun::Emoji {
                    emoji: e.to_owned(),
                    width: rw,
                    height: icon_h,
                });
                w += rw;
                h = h.max(icon_h);
            }
        }
    }

    PreparedName {
        size: Vec2::new(w, h),
        runs: prepared_runs,
    }
}

fn paint_prepared_runs_with_glow(
    painter: &egui::Painter,
    rect: Rect,
    prepared: &PreparedName,
    color: Color32,
    style: crate::theme::TextGlowStyle,
    reference_height: Option<f32>,
) {
    let mut x = rect.left();
    let cy = rect.center().y;
    let ref_h = reference_height
        .filter(|h| *h > 0.0)
        .unwrap_or(prepared.size.y);

    for run in &prepared.runs {
        match run {
            PreparedRun::Text {
                galley,
                width,
                height,
            } => {
                let y = cy - height / 2.0;
                crate::theme::text_glow::paint_glow_galley(
                    painter,
                    Pos2::new(x, y),
                    galley.clone(),
                    color,
                    style,
                    Some(ref_h),
                );
                x += width;
            }
            PreparedRun::Emoji {
                emoji,
                width,
                height,
            } => {
                let r =
                    Rect::from_center_size(Pos2::new(x + height / 2.0, cy), Vec2::splat(*height));
                try_paint_emoji(painter, emoji, r, color);
                x += width;
            }
        }
    }
}

fn paint_prepared_runs(
    painter: &egui::Painter,
    rect: Rect,
    prepared: &PreparedName,
    color: Color32,
    outlined: bool,
) {
    let mut x = rect.left();
    let cy = rect.center().y;
    let shadow_color = Color32::from_black_alpha(color.a());

    for run in &prepared.runs {
        match run {
            PreparedRun::Text {
                galley,
                width,
                height,
            } => {
                let y = cy - height / 2.0;
                if outlined {
                    crate::theme::paint_premium_glow_galley(
                        painter,
                        Pos2::new(x, y),
                        galley.clone(),
                        color,
                        shadow_color,
                    );
                } else {
                    painter.galley(Pos2::new(x, y), galley.clone(), color);
                }
                x += width;
            }
            PreparedRun::Emoji {
                emoji,
                width,
                height,
            } => {
                let r =
                    Rect::from_center_size(Pos2::new(x + height / 2.0, cy), Vec2::splat(*height));
                try_paint_emoji(painter, emoji, r, color);
                x += width;
            }
        }
    }
}

/// Paint a pre-prepared name with zero layout cost.
pub fn paint_prepared_name(
    painter: &egui::Painter,
    pos: Pos2,
    anchor: Align2,
    prepared: &PreparedName,
    color: Color32,
    outlined: bool,
) {
    let rect = anchor.anchor_size(pos, prepared.size);
    paint_prepared_runs(painter, rect, prepared, color, outlined);
}

/// Map nameplate glow — shared outline width via `reference_height` (e.g. name line height).
pub fn paint_prepared_name_with_glow(
    painter: &egui::Painter,
    pos: Pos2,
    anchor: Align2,
    prepared: &PreparedName,
    color: Color32,
    style: crate::theme::TextGlowStyle,
    reference_height: Option<f32>,
) {
    let rect = anchor.anchor_size(pos, prepared.size);
    paint_prepared_runs_with_glow(painter, rect, prepared, color, style, reference_height);
}

/// Draw a pixel emoji from the embedded atlas. Returns false if the glyph is not in the atlas.
pub fn try_paint_emoji(painter: &egui::Painter, emoji: &str, rect: Rect, tint: Color32) -> bool {
    let Some(uv) = crate::atlas_uv(emoji) else {
        return false;
    };
    let Some(texture) = crate::atlas_texture(painter.ctx()) else {
        return false;
    };

    let alpha = (tint.a() as f32 * 0.40) as u8;
    if alpha > 0 {
        let shadow_tint = Color32::from_black_alpha(alpha);
        let h = rect.height();
        let outline_width = (h * 0.04).clamp(0.4, 1.5);
        let shadow_dy = (h * 0.08).clamp(0.8, 3.0);
        let offsets = [
            (-outline_width, -outline_width),
            (outline_width, -outline_width),
            (-outline_width, outline_width),
            (outline_width, outline_width),
            (0.0, shadow_dy),
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
    let prepared = prepare_name(ui.painter(), text, &font_id);
    let (rect, response) = ui.allocate_exact_size(prepared.size, Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_prepared_runs(ui.painter(), rect, &prepared, color, false);
    }
    response
}

/// Outlined inline label (text parts get glow; emoji painted from atlas).
pub fn outlined_emoji_label(ui: &mut Ui, text: &str, font_id: FontId, color: Color32) -> Response {
    let prepared = prepare_name(ui.painter(), text, &font_id);
    let (rect, response) = ui.allocate_exact_size(prepared.size, Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_prepared_runs(ui.painter(), rect, &prepared, color, true);
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
    prepare_name(painter, text, font_id).size
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
    let prepared = prepare_name(painter, text, &font_id);
    paint_prepared_name(painter, pos, anchor, &prepared, color, outlined);
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
        let size = self.dim.unwrap_or(if cfg!(target_os = "android") {
            48.0
        } else {
            32.0
        });
        let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), Sense::click());

        let is_hovered = response.hovered();
        let is_active = response.is_pointer_button_down_on();
        let hover_t = ui.ctx().animate_bool(response.id.with("hover"), is_hovered);
        let active_t = ui.ctx().animate_bool(response.id.with("active"), is_active);
        let glow = (hover_t + active_t).min(1.0);
        if glow > 0.0 {
            let glow_color = crate::theme::palette::neon_cyan_glow();
            for i in 1..=3 {
                let t = i as f32 / 3.0;
                let factor = (-2.5 * t).exp();
                let a = (glow_color.a() as f32 * factor * glow) as u8;
                if a > 0 {
                    let step_color = Color32::from_rgba_unmultiplied(
                        glow_color.r(), glow_color.g(), glow_color.b(), a,
                    );
                    let offset = i as f32 * 2.0;
                    let expanded_rect = rect.expand(offset);
                    let cr = egui::CornerRadius::same((6.0 + offset).round() as u8);
                    ui.painter().rect_stroke(
                        expanded_rect,
                        cr,
                        egui::Stroke::new(1.0 + i as f32 * 1.0, step_color),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        }

        let emoji_size = size * 0.65;
        if !try_paint_emoji(
            ui.painter(),
            &self.emoji,
            Rect::from_center_size(rect.center(), egui::vec2(emoji_size, emoji_size)),
            self.color,
        ) {
            let font_id = egui::FontId::proportional(crate::theme::hud_button_text_size());
            crate::theme::paint_premium_glow_text(
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    Gold,
    Troops,
}

pub struct ResourceLabel {
    kind: ResourceKind,
    amount: f64,
    font_id: FontId,
    color: Option<Color32>,
    outlined: bool,
}

impl ResourceLabel {
    pub fn gold(amount: f64) -> Self {
        Self {
            kind: ResourceKind::Gold,
            amount,
            font_id: FontId::proportional(14.0),
            color: None,
            outlined: true,
        }
    }

    pub fn troops(amount: f64) -> Self {
        Self {
            kind: ResourceKind::Troops,
            amount,
            font_id: FontId::proportional(14.0),
            color: None,
            outlined: true,
        }
    }

    pub fn font(mut self, font_id: FontId) -> Self {
        self.font_id = font_id;
        self
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    pub fn outlined(mut self, outlined: bool) -> Self {
        self.outlined = outlined;
        self
    }
}

impl Widget for ResourceLabel {
    fn ui(self, ui: &mut Ui) -> Response {
        let (emoji, default_color) = match self.kind {
            ResourceKind::Gold => ("🪙", crate::theme::palette::neon_gold_hover()),
            ResourceKind::Troops => ("🛡️", crate::theme::palette::neon_cyan_hover()),
        };
        let color = self.color.unwrap_or(default_color);
        let text = format!("{} {}", emoji, crate::utils::format_number(self.amount));
        if self.outlined {
            outlined_emoji_label(ui, &text, self.font_id, color)
        } else {
            emoji_label(ui, &text, self.font_id, color)
        }
    }
}
