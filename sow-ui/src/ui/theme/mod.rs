use std::sync::Arc;

use egui::{
    style::{Selection, WidgetVisuals, Widgets},
    Align2, Color32, Context, CornerRadius, FontId, Galley, Image, Margin, Pos2, Rangef, Rect,
    Response, Sense, Stroke, StrokeKind, Style, TextStyle, TextureHandle, Ui, Vec2, Visuals,
};

/// Phone/narrow layout: stacked main menu + mobile leader art.
///
/// Uses width and height together so short-wide portal embeds (e.g. CrazyGames desktop QA)
/// stay on desktop layout when `width >= 600` even if `height < 600`.
#[inline]
pub fn compact_viewport(ctx: &Context) -> bool {
    let rect = ctx.input(|i| i.content_rect());
    let w = rect.width();
    let h = rect.height();
    w < 768.0 || h < 600.0 || w < h
}

/// Scale factor for fixed chrome (buttons, gaps, profile bar) on short viewports.
#[inline]
pub fn viewport_scale(ctx: &Context) -> f32 {
    (ctx.input(|i| i.content_rect()).height() / 720.0).clamp(0.55, 1.0)
}

/// Outer width of the main-menu left rail (content + optional glass frame inset).
///
/// Scales with [`viewport_scale`] so embedded portals don't clip button text.
#[inline]
pub fn menu_rail_panel_width(available_w: f32, compact: bool, ctx: &Context) -> f32 {
    let s = viewport_scale(ctx);
    if compact {
        return available_w.min(360.0 * s);
    }
    // Fixed ~340px content column at 1× scale; shrinks proportionally on short viewports.
    (340.0 * s).min(available_w)
}

/// Standard UI animation duration; near-instant when reduced motion is enabled.
#[inline]
pub fn anim_duration(reduced_motion: bool) -> f32 {
    if reduced_motion {
        0.01
    } else {
        0.22
    }
}

/// Reads `sow_reduced_motion` temp data set at frame start from [`SettingsState`].
#[inline]
pub fn anim_duration_from_ctx(ctx: &Context) -> f32 {
    let reduced = ctx
        .data(|d| d.get_temp::<bool>(egui::Id::new("sow_reduced_motion")))
        .unwrap_or(false);
    anim_duration(reduced)
}

/// Hover-scale animation duration (slightly snappier than panel transitions).
#[inline]
pub fn anim_duration_hover_from_ctx(ctx: &Context) -> f32 {
    let base = anim_duration_from_ctx(ctx);
    if base <= 0.02 {
        base
    } else {
        (base * 0.68).max(0.08)
    }
}

/// Publish reduced-motion preference for UI animation helpers this frame.
#[inline]
pub fn publish_reduced_motion(ctx: &Context, reduced_motion: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("sow_reduced_motion"), reduced_motion));
}

/// Transparent modal close control (settings, credits).
pub fn modal_close_button(ui: &mut Ui) -> Response {
    ui.add(
        crate::widgets::ThemeButton::new("✖")
            .style(crate::widgets::ThemeButtonStyle::Tertiary)
            .custom_fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .text_size(20.0)
            .custom_text_color(palette::text_muted()),
    )
}

/// Cosmic Rush palette
pub mod palette;


/// Layout tokens — use these instead of magic numbers in HUD code.
pub mod radius;


pub mod stroke;


pub mod margin;

pub mod tab;
mod panels;
mod tabs;
mod typography;

pub use panels::*;
pub use tabs::*;
pub use typography::*;

pub fn menu_backdrop() -> Color32 {
    palette::backdrop()
}
#[inline]
pub fn panel_bg() -> Color32 {
    palette::surface()
}
#[inline]
pub fn panel_bg_transparent() -> Color32 {
    palette::surface_transparent()
}
#[inline]
pub fn screen_bg() -> Color32 {
    Color32::from_rgb(8, 10, 14)
}
#[inline]
pub fn menu_panel_border_glow() -> Color32 {
    palette::neon_cyan_glow()
}
#[inline]
pub fn accent_solo_cyan() -> Color32 {
    palette::neon_cyan()
}
#[inline]
pub fn accent_solo_cyan_hover() -> Color32 {
    palette::neon_cyan_hover()
}
#[inline]
pub fn accent_ranked_gold() -> Color32 {
    palette::neon_gold()
}
#[inline]
pub fn accent_ranked_gold_hover() -> Color32 {
    palette::neon_gold_hover()
}
#[inline]
pub fn menu_secondary_button() -> Color32 {
    palette::button_inactive()
}
#[inline]
pub fn menu_secondary_button_hover() -> Color32 {
    palette::button_hovered()
}
#[inline]
pub fn nickname_field_bg() -> Color32 {
    palette::field_bg()
}
#[inline]
pub fn nickname_field_border() -> Color32 {
    palette::field_border()
}
#[inline]
pub fn accent_danger() -> Color32 {
    palette::danger()
}
#[inline]
pub fn accent_danger_border() -> Color32 {
    palette::danger_border()
}
#[inline]
pub fn avatar_pink() -> Color32 {
    palette::pink()
}
#[inline]
pub fn text_secondary() -> Color32 {
    palette::text_muted()
}

/// Draw text with a crisp black outline and heavy bottom drop shadow.
///
/// Convenience wrapper — delegates to [`paint_premium_glow_text`].
pub fn font_regular(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name("Regular".into()))
}

pub fn apply_theme(ctx: &Context) {
    // PressStart2P only — do not pull in egui bundled default/emoji font blobs.
    let mut fonts = egui::FontDefinitions {
        font_data: Default::default(),
        families: Default::default(),
    };
    fonts.font_data.insert(
        "Default".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(crate::ui_font::UI_FONT_TTF)),
    );
    fonts.font_data.insert(
        "Regular".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(crate::ui_font::UI_REGULAR_FONT_TTF)),
    );
    fonts
        .families
        .insert(egui::FontFamily::Proportional, vec!["Default".to_owned()]);
    fonts
        .families
        .insert(egui::FontFamily::Monospace, vec!["Default".to_owned()]);
    fonts
        .families
        .insert(egui::FontFamily::Name("Regular".into()), vec!["Regular".to_owned()]);
    ctx.set_fonts(fonts);

    let mut style = Style {
        text_styles: [
            (TextStyle::Heading, FontId::proportional(32.0)),
            (TextStyle::Body, FontId::proportional(18.0)),
            (TextStyle::Monospace, FontId::monospace(14.0)),
            (TextStyle::Button, FontId::proportional(20.0)),
            (TextStyle::Small, FontId::proportional(14.0)),
        ]
        .into(),
        ..Default::default()
    };

    // Refined Spacing inspired by egui best practices
    style.spacing.item_spacing = egui::vec2(16.0, 16.0); // More roomy
    style.spacing.button_padding = egui::vec2(24.0, 12.0); // Large touch targets
    style.spacing.window_margin = Margin::same(20);
    style.spacing.menu_margin = Margin::same(12);

    let mut visuals = Visuals::dark();
    visuals.window_fill = palette::backdrop();
    visuals.panel_fill = palette::surface();
    visuals.faint_bg_color = palette::button_inactive();

    // Cosmic Drop Shadows
    visuals.window_shadow = egui::Shadow {
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(180),
        offset: [0, 12],
    };
    visuals.popup_shadow = egui::Shadow {
        blur: 24,
        spread: 0,
        color: palette::neon_cyan_glow(),
        offset: [0, 10],
    };

    visuals.override_text_color = Some(palette::text_normal());
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(12);

    // Overhaul Widgets definitions
    visuals.widgets = Widgets {
        noninteractive: WidgetVisuals {
            weak_bg_fill: palette::field_bg(),
            bg_fill: palette::field_bg(),
            bg_stroke: Stroke::new(1.0_f32, palette::field_border()),
            fg_stroke: Stroke::new(1.0_f32, palette::text_muted()),
            corner_radius: CornerRadius::same(12),
            expansion: 0.0,
        },
        inactive: WidgetVisuals {
            weak_bg_fill: palette::button_inactive(),
            bg_fill: palette::button_inactive(),
            bg_stroke: Stroke::new(1.0_f32, palette::field_border()),
            fg_stroke: Stroke::new(1.0_f32, palette::text_normal()),
            corner_radius: CornerRadius::same(12),
            expansion: 0.0,
        },
        hovered: WidgetVisuals {
            weak_bg_fill: palette::button_hovered(),
            bg_fill: palette::button_hovered(),
            bg_stroke: Stroke::new(1.5_f32, palette::neon_cyan_hover()),
            fg_stroke: Stroke::new(1.5_f32, Color32::WHITE),
            corner_radius: CornerRadius::same(12),
            expansion: 1.0,
        },
        active: WidgetVisuals {
            weak_bg_fill: palette::neon_cyan(),
            bg_fill: palette::neon_cyan(),
            bg_stroke: Stroke::new(2.0_f32, palette::neon_cyan_hover()),
            fg_stroke: Stroke::new(2.0_f32, Color32::WHITE),
            corner_radius: CornerRadius::same(12),
            expansion: 2.0,
        },
        open: WidgetVisuals {
            weak_bg_fill: palette::button_hovered(),
            bg_fill: palette::button_hovered(),
            bg_stroke: Stroke::new(1.0_f32, palette::field_border()),
            fg_stroke: Stroke::new(1.0_f32, palette::text_normal()),
            corner_radius: CornerRadius::same(12),
            expansion: 0.0,
        },
    };

    visuals.selection = Selection {
        bg_fill: palette::neon_cyan_glow(),
        stroke: Stroke::new(1.0_f32, palette::neon_cyan_hover()),
    };

    style.visuals = visuals;
    ctx.set_global_style(style);
}
