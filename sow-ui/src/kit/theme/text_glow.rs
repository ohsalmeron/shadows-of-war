use std::sync::Arc;

use egui::{Color32, Galley, Painter, Pos2, Vec2};

/// Outline ring + downward drop shadow for map/HUD text.
#[derive(Clone, Copy, Debug)]
pub struct TextGlowStyle {
    pub outline_width_ratio: f32,
    pub outline_width_min: f32,
    pub outline_width_max: f32,
    pub shadow_dy_ratio: f32,
    pub shadow_dy_min: f32,
    pub shadow_dy_max: f32,
    pub outline_alpha: u8,
    pub shadow_alpha: u8,
}

impl TextGlowStyle {
    pub const fn outline_width(self, reference_height: f32) -> f32 {
        let h = reference_height.max(0.0);
        (h * self.outline_width_ratio).clamp(self.outline_width_min, self.outline_width_max)
    }

    pub const fn shadow_dy(self, reference_height: f32) -> f32 {
        let h = reference_height.max(0.0);
        (h * self.shadow_dy_ratio).clamp(self.shadow_dy_min, self.shadow_dy_max)
    }

    pub fn outline_color(self) -> Color32 {
        Color32::from_black_alpha(self.outline_alpha)
    }

    pub fn shadow_color(self) -> Color32 {
        Color32::from_black_alpha(self.shadow_alpha)
    }
}

/// HUD menus and panels — matches legacy premium glow math (opaque caller shadow).
pub const HUD_PREMIUM: TextGlowStyle = TextGlowStyle {
    outline_width_ratio: 0.05,
    outline_width_min: 0.4,
    outline_width_max: 1.5,
    shadow_dy_ratio: 0.08,
    shadow_dy_min: 0.8,
    shadow_dy_max: 2.5,
    outline_alpha: 255,
    shadow_alpha: 255,
};

/// Lighter regular-weight HUD variant.
pub const HUD_PREMIUM_REGULAR: TextGlowStyle = TextGlowStyle {
    outline_width_ratio: 0.04,
    outline_width_min: 0.4,
    outline_width_max: 1.2,
    shadow_dy_ratio: 0.07,
    shadow_dy_min: 0.6,
    shadow_dy_max: 2.0,
    outline_alpha: 255,
    shadow_alpha: 255,
};

/// Map nameplates — shared outline width via reference height; soft alpha.
pub const NAMEPLATE: TextGlowStyle = TextGlowStyle {
    outline_width_ratio: 0.05,
    outline_width_min: 0.4,
    outline_width_max: 1.2,
    shadow_dy_ratio: 0.08,
    shadow_dy_min: 0.8,
    shadow_dy_max: 2.5,
    outline_alpha: 200,
    shadow_alpha: 140,
};

#[inline]
fn resolve_reference_height(galley: &Galley, reference_height: Option<f32>) -> f32 {
    reference_height
        .filter(|h| *h > 0.0)
        .unwrap_or_else(|| galley.rect.height())
}

/// 1 shadow (down Y) + 4 diagonal outline + core fill.
pub fn paint_glow_galley(
    painter: &Painter,
    pos: Pos2,
    galley: Arc<Galley>,
    base_color: Color32,
    style: TextGlowStyle,
    reference_height: Option<f32>,
) {
    if galley.is_empty() {
        return;
    }
    paint_glow_galley_colors(
        painter,
        pos,
        galley,
        base_color,
        style,
        reference_height,
        (None, None),
    );
}

pub fn paint_glow_galley_colors(
    painter: &Painter,
    pos: Pos2,
    galley: Arc<Galley>,
    base_color: Color32,
    style: TextGlowStyle,
    reference_height: Option<f32>,
    color_overrides: (Option<Color32>, Option<Color32>),
) {
    let (outline_override, shadow_override) = color_overrides;
    if galley.is_empty() {
        return;
    }

    let h = resolve_reference_height(&galley, reference_height);
    let outline_w = style.outline_width(h);
    let shadow_dy = style.shadow_dy(h);
    let outline_col = outline_override.unwrap_or_else(|| style.outline_color());
    let shadow_col = shadow_override.unwrap_or_else(|| style.shadow_color());

    painter.galley_with_override_text_color(
        pos + Vec2::new(0.0, shadow_dy),
        galley.clone(),
        shadow_col,
    );

    for &(dx, dy) in &[
        (-outline_w, -outline_w),
        (outline_w, -outline_w),
        (-outline_w, outline_w),
        (outline_w, outline_w),
    ] {
        painter.galley_with_override_text_color(
            pos + Vec2::new(dx, dy),
            galley.clone(),
            outline_col,
        );
    }

    painter.galley_with_override_text_color(pos, galley, base_color);
}
