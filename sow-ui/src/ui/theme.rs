use egui::{Color32, Context, CornerRadius, FontId, Margin, Stroke, Style, TextStyle, Visuals};

/// Cosmic Rush palette
#[inline]
pub fn menu_backdrop() -> Color32 {
    Color32::from_rgb(10, 10, 15) // var(--cosmic-bg-darkest)
}

#[inline]
pub fn panel_bg() -> Color32 {
    Color32::from_rgb(17, 24, 39) // var(--cosmic-bg-darker)
}

#[inline]
pub fn panel_bg_transparent() -> Color32 {
    Color32::from_rgba_unmultiplied(17, 24, 39, 200)
}

#[inline]
pub fn menu_panel_border_glow() -> Color32 {
    Color32::from_rgba_unmultiplied(6, 182, 212, 140) // Cyan glow
}

#[inline]
pub fn accent_solo_cyan() -> Color32 {
    Color32::from_rgb(6, 182, 212) // var(--cosmic-cyan)
}

#[inline]
pub fn accent_solo_cyan_hover() -> Color32 {
    Color32::from_rgb(34, 211, 238) // Brighter cyan
}

#[inline]
pub fn accent_ranked_gold() -> Color32 {
    Color32::from_rgb(234, 179, 8) // var(--cosmic-yellow)
}

#[inline]
pub fn accent_ranked_gold_hover() -> Color32 {
    Color32::from_rgb(250, 204, 21) // Brighter yellow
}

#[inline]
pub fn menu_secondary_button() -> Color32 {
    Color32::from_rgb(31, 41, 55) // var(--cosmic-bg-dark)
}

#[inline]
pub fn menu_secondary_button_hover() -> Color32 {
    Color32::from_rgb(55, 65, 81) // Lighter cosmic gray
}

#[inline]
pub fn nickname_field_bg() -> Color32 {
    Color32::from_rgb(17, 24, 39) // var(--cosmic-bg-darker)
}

#[inline]
pub fn nickname_field_border() -> Color32 {
    Color32::from_rgb(55, 65, 81) // Border subtle gray
}

#[inline]
pub fn accent_danger() -> Color32 {
    Color32::from_rgb(239, 68, 68) // var(--cosmic-red)
}

#[inline]
pub fn accent_danger_border() -> Color32 {
    Color32::from_rgb(220, 38, 38)
}

#[inline]
pub fn avatar_pink() -> Color32 {
    Color32::from_rgb(236, 72, 153) // var(--cosmic-pink)
}

#[inline]
pub fn avatar_cyan() -> Color32 {
    Color32::from_rgb(34, 211, 238)
}

#[inline]
pub fn text_secondary() -> Color32 {
    Color32::from_rgb(156, 163, 175) // var(--cosmic-gray)
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

    style.spacing.item_spacing = egui::vec2(16.0, 16.0); // More roomy
    style.spacing.button_padding = egui::vec2(20.0, 12.0);
    style.spacing.window_margin = Margin::same(16);

    ctx.set_global_style(style);

    let mut visuals = Visuals::dark();
    visuals.window_fill = menu_backdrop();
    visuals.panel_fill = panel_bg();
    visuals.faint_bg_color = menu_secondary_button();
    
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
        color: Color32::from_rgba_unmultiplied(6, 182, 212, 40),
        offset: [0, 10],
    };

    visuals.override_text_color = Some(Color32::from_rgb(243, 244, 246));

    let neon_cyan = accent_solo_cyan();
    let neon_cyan_hover = accent_solo_cyan_hover();
    let panel_border = nickname_field_border();

    visuals.widgets.noninteractive.bg_fill = nickname_field_bg();
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, panel_border);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(12);

    visuals.widgets.inactive.bg_fill = menu_secondary_button();
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, panel_border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(243, 244, 246));
    visuals.widgets.inactive.corner_radius = CornerRadius::same(12);

    visuals.widgets.hovered.bg_fill = menu_secondary_button_hover();
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, neon_cyan_hover);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(12);

    visuals.widgets.active.bg_fill = neon_cyan;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, neon_cyan);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.active.corner_radius = CornerRadius::same(12);

    ctx.set_visuals(visuals);
}
