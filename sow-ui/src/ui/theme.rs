use egui::{
    style::{Selection, WidgetVisuals, Widgets},
    Color32, Context, CornerRadius, FontId, Margin, Stroke, Style, TextStyle, Visuals,
};

/// Cosmic Rush palette
pub mod palette {
    use egui::Color32;

    #[inline]
    pub fn backdrop() -> Color32 {
        Color32::from_black_alpha(120)
    } // Glassmorphism backdrop
    #[inline]
    pub fn surface() -> Color32 {
        Color32::from_rgba_unmultiplied(12, 12, 14, 140)
    } // Translucent glass base
    #[inline]
    pub fn surface_transparent() -> Color32 {
        Color32::from_rgba_unmultiplied(12, 12, 14, 100)
    }

    #[inline]
    pub fn neon_cyan() -> Color32 {
        Color32::from_rgb(6, 182, 212)
    } // var(--cosmic-cyan)
    #[inline]
    pub fn neon_cyan_hover() -> Color32 {
        Color32::from_rgb(34, 211, 238)
    } // Brighter cyan
    #[inline]
    pub fn neon_cyan_glow() -> Color32 {
        Color32::from_rgba_unmultiplied(6, 182, 212, 80)
    } // Electric blue glow

    #[inline]
    pub fn neon_gold() -> Color32 {
        Color32::from_rgb(234, 179, 8)
    } // var(--cosmic-yellow)
    #[inline]
    pub fn neon_gold_hover() -> Color32 {
        Color32::from_rgb(250, 204, 21)
    } // Brighter yellow

    #[inline]
    pub fn button_inactive() -> Color32 {
        Color32::from_rgba_unmultiplied(22, 22, 24, 120)
    }
    #[inline]
    pub fn button_hovered() -> Color32 {
        Color32::from_rgba_unmultiplied(40, 40, 44, 170)
    }

    #[inline]
    pub fn field_bg() -> Color32 {
        Color32::from_rgba_unmultiplied(8, 8, 10, 150)
    } // Deep inset glass
    #[inline]
    pub fn field_border() -> Color32 {
        Color32::from_rgba_unmultiplied(75, 85, 99, 80)
    } // Subtle slate border

    #[inline]
    pub fn danger() -> Color32 {
        Color32::from_rgb(239, 68, 68)
    } // var(--cosmic-red)
    #[inline]
    pub fn danger_border() -> Color32 {
        Color32::from_rgb(220, 38, 38)
    }

    #[inline]
    pub fn pink() -> Color32 {
        Color32::from_rgb(236, 72, 153)
    } // var(--cosmic-pink)

    #[inline]
    pub fn text_normal() -> Color32 {
        Color32::from_rgb(243, 244, 246)
    }
    #[inline]
    pub fn text_muted() -> Color32 {
        Color32::from_rgb(156, 163, 175)
    } // var(--cosmic-gray)
}

// Backward-compatible inline functions
#[inline]
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
pub fn avatar_cyan() -> Color32 {
    palette::neon_cyan_hover()
}
#[inline]
pub fn text_secondary() -> Color32 {
    palette::text_muted()
}

pub fn apply_theme(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Default".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(crate::ui_font::UI_FONT_TTF)),
    );

    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "Default".to_owned());
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

#[inline]
pub fn hud_panel_frame() -> egui::Frame {
    let (margin_x, margin_y) = if cfg!(target_os = "android") {
        (12, 6)
    } else {
        (8, 4)
    };

    egui::Frame::NONE
        .fill(Color32::from_black_alpha(150))
        .corner_radius(8.0)
        .stroke(egui::Stroke::new(1.0_f32, nickname_field_border()))
        .inner_margin(egui::Margin::symmetric(margin_x, margin_y))
}

#[inline]
pub fn standard_panel_frame(compact: bool) -> egui::Frame {
    if compact {
        egui::Frame::new()
            .fill(panel_bg())
            .stroke(egui::Stroke::NONE)
            .corner_radius(CornerRadius::ZERO)
            .inner_margin(egui::Margin::same(16))
            .shadow(egui::Shadow::NONE)
    } else {
        egui::Frame::new()
            .fill(panel_bg())
            .stroke(egui::Stroke::new(1.0_f32, menu_panel_border_glow()))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(egui::Margin::same(24))
            .shadow(egui::Shadow {
                blur: 24,
                spread: 0,
                color: Color32::from_rgba_unmultiplied(6, 182, 212, 30),
                offset: [0, 10],
            })
    }
}

#[inline]
pub fn hud_button_text_size() -> f32 {
    if cfg!(target_os = "android") {
        32.0
    } else {
        18.0
    }
}

/// Draw text with a crisp black outline and heavy bottom drop shadow.
///
/// Uses an optimized 7-pass style (2 dragged shadow passes, 4 diagonal outline passes, 1 core pass)
/// for a bold, game-style look with maximum rendering performance.
pub fn paint_premium_glow_text(
    painter: &egui::Painter,
    pos: egui::Pos2,
    anchor: egui::Align2,
    text: &str,
    font_id: egui::FontId,
    base_color: Color32,
    shadow_color: Color32,
) {
    if text.is_empty() {
        return;
    }
    let black = shadow_color;

    // 1. Dragged-down 3D Opaque Black Shadow (2 passes)
    for &dy in &[2.0, 4.0] {
        painter.text(
            pos + egui::vec2(0.0, dy),
            anchor,
            text,
            font_id.clone(),
            black,
        );
    }

    // 2. 4-way diagonal outline (4 passes)
    for &(dx, dy) in &[(-1.5, -1.5), (1.5, -1.5), (-1.5, 1.5), (1.5, 1.5)] {
        painter.text(
            pos + egui::vec2(dx, dy),
            anchor,
            text,
            font_id.clone(),
            black,
        );
    }

    // 3. Core text (1 pass)
    painter.text(pos, anchor, text, font_id, base_color);
}

/// Draw text with a crisp black outline and heavy bottom drop shadow.
///
/// Uses 5 shadow passes (L/R/T/B + extra bottom) for a bold, game-style look.
/// Only use on important, low-count text (titles, overlays, loading status).
/// For bulk text (hundreds of bot labels), use a simple 1-pass drop shadow instead.
pub fn outlined_text(
    painter: &egui::Painter,
    pos: egui::Pos2,
    anchor: egui::Align2,
    text: &str,
    font_id: egui::FontId,
    color: Color32,
    shadow_color: Color32,
) {
    paint_premium_glow_text(painter, pos, anchor, text, font_id, color, shadow_color);
}

/// A UI widget that draws text with an outline. Use this instead of `ui.label()` for important titles.
pub fn outlined_label(
    ui: &mut egui::Ui,
    text: &str,
    font_id: egui::FontId,
    color: Color32,
) -> egui::Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font_id.clone(), color);
    let (rect, response) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        outlined_text(
            ui.painter(),
            rect.left_top(),
            egui::Align2::LEFT_TOP,
            text,
            font_id,
            color,
            Color32::BLACK,
        );
    }
    response
}
