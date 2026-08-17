use super::*;

pub fn outlined_text(
    painter: &egui::Painter,
    pos: egui::Pos2,
    anchor: Align2,
    text: &str,
    font_id: FontId,
    color: Color32,
    shadow_color: Color32,
) {
    paint_premium_glow_text(painter, pos, anchor, text, font_id, color, shadow_color);
}

/// Paint a pre-laid-out galley with premium 7-pass glow (zero layout cost).
///
/// `pos` is the top-left anchor of the galley.
pub fn paint_premium_glow_galley(
    painter: &egui::Painter,
    pos: egui::Pos2,
    galley: Arc<Galley>,
    base_color: Color32,
    shadow_color: Color32,
) {
    super::text_glow::paint_glow_galley_colors(
        painter,
        pos,
        galley,
        base_color,
        super::text_glow::HUD_PREMIUM,
        None,
        (Some(shadow_color), Some(shadow_color)),
    );
}

/// Leader name in caps with crisp 7-pass black outline and clear size hierarchy vs body text.
pub fn leader_name_label(ui: &mut Ui, name: &str, size: f32) -> Response {
    leader_caps_line(ui, name, size)
}

/// All-caps white line with the standard 7-pass black outline and drop shadow.
pub fn leader_caps_line(ui: &mut Ui, text: &str, size: f32) -> Response {
    let text = text.to_uppercase();
    let font_id = FontId::proportional(size);
    let galley = ui.painter().layout_no_wrap(text, font_id, Color32::WHITE);
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_premium_glow_galley(
            ui.painter(),
            rect.left_top(),
            galley,
            Color32::WHITE,
            Color32::BLACK,
        );
    }
    response
}

/// Wrapped block with standard 7-pass black outline using the core WorkSans-Black font.
pub fn leader_caps_paragraph(ui: &mut Ui, text: &str, size: f32, wrap_w: f32) -> Response {
    let text = text.to_uppercase();
    let font_id = FontId::proportional(size);
    let galley = ui.painter().layout(text, font_id, Color32::WHITE, wrap_w);
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_premium_glow_galley(
            ui.painter(),
            rect.left_top(),
            galley,
            Color32::WHITE,
            Color32::BLACK,
        );
    }
    response
}

/// Paint regular weight font with a lighter/thinner outline and drop shadow.
pub fn paint_premium_glow_galley_regular(
    painter: &egui::Painter,
    pos: egui::Pos2,
    galley: Arc<Galley>,
    base_color: Color32,
    shadow_color: Color32,
) {
    super::text_glow::paint_glow_galley_colors(
        painter,
        pos,
        galley,
        base_color,
        super::text_glow::HUD_PREMIUM_REGULAR,
        None,
        (Some(shadow_color), Some(shadow_color.linear_multiply(0.7))),
    );
}

/// Draw text with a crisp black outline and heavy bottom drop shadow.
///
/// Layout-once + 7× galley paint. For callers that already have a galley,
/// use [`paint_premium_glow_galley`] directly.
pub fn paint_premium_glow_text(
    painter: &egui::Painter,
    pos: egui::Pos2,
    anchor: Align2,
    text: &str,
    font_id: FontId,
    base_color: Color32,
    shadow_color: Color32,
) {
    if text.is_empty() {
        return;
    }
    let is_regular = font_id.family == egui::FontFamily::Name("Regular".into());
    let galley = painter.layout_no_wrap(text.to_owned(), font_id, base_color);
    let anchor_pos = anchor_top_left(pos, anchor, galley.size());
    if is_regular {
        paint_premium_glow_galley_regular(painter, anchor_pos, galley, base_color, shadow_color);
    } else {
        paint_premium_glow_galley(painter, anchor_pos, galley, base_color, shadow_color);
    }
}

/// Resolve an `Align2` anchor + size into the top-left position egui galley expects.
#[inline]
fn anchor_top_left(pos: egui::Pos2, anchor: Align2, size: egui::Vec2) -> egui::Pos2 {
    let x = match anchor.0[0] {
        egui::Align::Min => pos.x,
        egui::Align::Center => pos.x - size.x * 0.5,
        egui::Align::Max => pos.x - size.x,
    };
    let y = match anchor.0[1] {
        egui::Align::Min => pos.y,
        egui::Align::Center => pos.y - size.y * 0.5,
        egui::Align::Max => pos.y - size.y,
    };
    egui::pos2(x, y)
}

/// A UI widget that draws text with an outline. Lays out once, paints 7×.
pub fn outlined_label(
    ui: &mut egui::Ui,
    text: &str,
    font_id: FontId,
    color: Color32,
) -> egui::Response {
    let is_regular = font_id.family == egui::FontFamily::Name("Regular".into());
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font_id, color);
    let (rect, response) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        if is_regular {
            paint_premium_glow_galley_regular(
                ui.painter(),
                rect.left_top(),
                galley,
                color,
                Color32::BLACK,
            );
        } else {
            paint_premium_glow_galley(ui.painter(), rect.left_top(), galley, color, Color32::BLACK);
        }
    }
    response
}
