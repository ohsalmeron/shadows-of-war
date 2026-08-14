use super::*;

pub struct TabStyle {
    pub inactive_fill: Color32,
    pub hover_fill: Color32,
    pub active_fill: Color32,
    pub baseline: Color32,
    pub label_active: Color32,
    pub label_inactive: Color32,
}

#[inline]
pub fn hud_tab_style() -> TabStyle {
    TabStyle {
        inactive_fill: palette::button_inactive(),
        hover_fill: palette::button_hovered(),
        active_fill: palette::field_bg(),
        baseline: palette::field_border(),
        label_active: palette::text_normal(),
        label_inactive: palette::text_muted(),
    }
}
pub enum TabContent<'a> {
    Text(&'a str),
    Icon(Option<&'a TextureHandle>),
}

/// Browser-style tab: rounded top, flat bottom, accent stripe when selected.
pub fn draw_tab(
    ui: &mut Ui,
    content: TabContent,
    selected: bool,
    accent: Color32,
    badge_count: usize,
    tab_w: f32,
    compact: bool,
) -> Response {
    let style = hud_tab_style();
    let tab_h = tab::height(compact);
    let tab_radius = radius::tab_top();
    let font_size = if compact { 10.0 } else { 11.0 };
    let icon_size = if compact { 20.0 } else { 22.0 };

    let (rect, response) = ui.allocate_exact_size(Vec2::new(tab_w, tab_h), Sense::click());

    if ui.is_rect_visible(rect) {
        let fill = if selected {
            style.active_fill
        } else if response.hovered() {
            style.hover_fill
        } else {
            style.inactive_fill
        };

        let side_stroke = if selected {
            Stroke::new(stroke::HAIRLINE, accent.linear_multiply(0.6))
        } else {
            Stroke::NONE
        };

        ui.painter()
            .rect(rect, tab_radius, fill, side_stroke, StrokeKind::Inside);

        if selected {
            let bar = Rect::from_min_max(
                Pos2::new(rect.left() + 1.0, rect.top()),
                Pos2::new(rect.right() - 1.0, rect.top() + tab::ACCENT_BAR_H),
            );
            ui.painter().rect_filled(bar, 0, accent);
        } else {
            let baseline_y = rect.bottom() - tab::BASELINE_H;
            let baseline_stroke = if response.hovered() {
                Stroke::new(stroke::EMPHASIS, accent.linear_multiply(0.85))
            } else {
                Stroke::new(stroke::HAIRLINE, style.baseline.linear_multiply(0.7))
            };
            ui.painter()
                .hline(rect.x_range(), baseline_y, baseline_stroke);
        }

        match content {
            TabContent::Text(label) => {
                let label_color = if selected {
                    style.label_active
                } else {
                    style.label_inactive
                };
                crate::widgets::paint_emoji_text_at(
                    ui.painter(),
                    rect.center(),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(font_size),
                    label_color,
                    false,
                );
            }
            TabContent::Icon(Some(tex)) => {
                let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(icon_size));
                ui.put(
                    icon_rect,
                    Image::new(tex).fit_to_exact_size(icon_rect.size()),
                );
            }
            TabContent::Icon(None) => {}
        }

        if badge_count > 0 {
            let badge_center = rect.right_top() + Vec2::new(-6.0, 6.0);
            let badge_r = if compact { 5.5 } else { 6.0 };
            ui.painter()
                .circle_filled(badge_center, badge_r, palette::danger());
            let badge_text = if badge_count > 9 {
                "9+".to_string()
            } else {
                badge_count.to_string()
            };
            ui.painter().text(
                badge_center,
                Align2::CENTER_CENTER,
                badge_text,
                FontId::proportional(if compact { 7.0 } else { 7.5 }),
                Color32::WHITE,
            );
        }
    }

    response
}

/// Baseline under the tab strip; skips the active tab's bottom edge.
pub fn draw_tab_baseline(ui: &mut Ui, strip_rect: Rect, active_tab_rect: Option<Rect>) {
    let y = strip_rect.bottom() - tab::BASELINE_H;
    let baseline = Stroke::new(
        stroke::HAIRLINE,
        palette::field_border().linear_multiply(0.85),
    );

    if let Some(active) = active_tab_rect {
        let left = strip_rect.x_range();
        if active.left() > left.min + 1.0 {
            ui.painter()
                .hline(Rangef::new(left.min, active.left()), y, baseline);
        }
        if active.right() < left.max - 1.0 {
            ui.painter()
                .hline(Rangef::new(active.right(), left.max), y, baseline);
        }
    } else {
        ui.painter().hline(strip_rect.x_range(), y, baseline);
    }
}
