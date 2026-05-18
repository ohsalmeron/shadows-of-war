use egui::{
    Context, Color32, CornerRadius, FontId, Margin, Stroke, Style, TextStyle, Visuals,
};

/// Dark Rift palette — see `dark-rift/crates/client/src/ui/theme.rs`.
#[inline]
pub fn menu_backdrop() -> Color32 {
    Color32::from_rgb(35, 42, 35) // Dark forest (lightened)
}

#[inline]
pub fn panel_bg() -> Color32 {
    Color32::from_rgb(48, 60, 48) // Moss panel (lightened)
}

#[inline]
pub fn panel_bg_transparent() -> Color32 {
    Color32::from_rgba_unmultiplied(48, 60, 48, 200) // Moss panel with transparency
}

#[inline]
pub fn menu_panel_border_glow() -> Color32 {
    Color32::from_rgba_unmultiplied(61, 92, 61, 140) // Soft green glow
}

#[inline]
pub fn accent_solo_cyan() -> Color32 {
    Color32::from_rgb(78, 127, 78) // Mossy accent
}

#[inline]
pub fn accent_solo_cyan_hover() -> Color32 {
    Color32::from_rgb(105, 165, 105) // Lighter moss
}

#[inline]
pub fn accent_ranked_gold() -> Color32 {
    Color32::from_rgb(112, 90, 49) // Dark wood
}

#[inline]
pub fn accent_ranked_gold_hover() -> Color32 {
    Color32::from_rgb(152, 123, 68) // Lighter wood
}

#[inline]
pub fn menu_secondary_button() -> Color32 {
    Color32::from_rgb(60, 75, 60) // Lighter panel (lightened)
}

#[inline]
pub fn menu_secondary_button_hover() -> Color32 {
    Color32::from_rgb(75, 95, 75)
}

#[inline]
pub fn nickname_field_bg() -> Color32 {
    Color32::from_rgb(25, 32, 25) // Very dark input (lightened)
}

#[inline]
pub fn nickname_field_border() -> Color32 {
    Color32::from_rgb(75, 95, 75)
}

#[inline]
pub fn accent_danger() -> Color32 {
    Color32::from_rgb(110, 59, 59) // Organic red
}

#[inline]
pub fn accent_danger_border() -> Color32 {
    Color32::from_rgb(140, 74, 74)
}

#[inline]
pub fn text_secondary() -> Color32 {
    Color32::from_rgb(136, 162, 136) // Sage text
}

pub fn apply_theme(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Default".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(crate::ui_font::UI_FONT_TTF)),
    );

    fonts.families
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

    style.spacing.item_spacing = egui::vec2(12.0, 12.0);
    style.spacing.button_padding = egui::vec2(18.0, 10.0);
    style.spacing.window_margin = Margin::same(14);

    ctx.set_global_style(style);

    let mut visuals = Visuals::dark();
    visuals.window_fill = menu_backdrop();
    visuals.panel_fill = panel_bg();
    visuals.faint_bg_color = menu_secondary_button();

    visuals.override_text_color = Some(Color32::from_rgb(220, 230, 220)); 

    let neon_cyan = accent_solo_cyan();
    let neon_cyan_hover = accent_solo_cyan_hover();
    let panel_border = nickname_field_border();

    visuals.widgets.noninteractive.bg_fill = nickname_field_bg();
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, panel_border);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(12);

    visuals.widgets.inactive.bg_fill = menu_secondary_button();
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, panel_border);
    visuals.widgets.inactive.fg_stroke =
        Stroke::new(1.0_f32, Color32::from_rgb(220, 230, 220));
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
