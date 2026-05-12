use egui::{
    Context, Color32, CornerRadius, FontId, Margin, Stroke, Style, TextStyle, Visuals,
};

/// Shadows of War palette — see `dark-rift/crates/client/src/ui/theme.rs`.

#[inline]
pub fn menu_backdrop() -> Color32 {
    Color32::from_rgba_unmultiplied(10, 20, 29, 247)
}

#[inline]
pub fn panel_bg() -> Color32 {
    Color32::from_rgba_unmultiplied(25, 28, 36, 230)
}

#[inline]
pub fn menu_panel_border_glow() -> Color32 {
    Color32::from_rgba_unmultiplied(0, 211, 255, 140)
}

#[inline]
pub fn accent_solo_cyan() -> Color32 {
    Color32::from_rgb(0, 211, 255)
}

#[inline]
pub fn accent_solo_cyan_hover() -> Color32 {
    Color32::from_rgb(51, 235, 255)
}

#[inline]
pub fn accent_ranked_gold() -> Color32 {
    Color32::from_rgb(191, 148, 64)
}

#[inline]
pub fn accent_ranked_gold_hover() -> Color32 {
    Color32::from_rgb(224, 184, 89)
}

#[inline]
pub fn menu_secondary_button() -> Color32 {
    Color32::from_rgba_unmultiplied(36, 41, 51, 242)
}

#[inline]
pub fn menu_secondary_button_hover() -> Color32 {
    Color32::from_rgba_unmultiplied(56, 66, 82, 250)
}

#[inline]
pub fn nickname_field_bg() -> Color32 {
    Color32::from_rgba_unmultiplied(15, 23, 31, 242)
}

#[inline]
pub fn nickname_field_border() -> Color32 {
    Color32::from_rgba_unmultiplied(0, 166, 230, 115)
}

#[inline]
pub fn accent_danger() -> Color32 {
    Color32::from_rgba_unmultiplied(133, 51, 46, 224)
}

#[inline]
pub fn accent_danger_border() -> Color32 {
    Color32::from_rgba_unmultiplied(242, 128, 115, 217)
}

#[inline]
pub fn text_secondary() -> Color32 {
    Color32::from_rgba_unmultiplied(184, 209, 240, 242)
}

pub fn apply_theme(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        sow_core::ui_font::UI_FONT_FAMILY.to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(sow_core::ui_font::UI_FONT_TTF)),
    );
    fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, sow_core::ui_font::UI_FONT_FAMILY.to_owned());
    ctx.set_fonts(fonts);

    let mut style = Style::default();

    style.text_styles = [
        (TextStyle::Heading, FontId::proportional(32.0)),
        (TextStyle::Body, FontId::proportional(18.0)),
        (TextStyle::Monospace, FontId::monospace(14.0)),
        (TextStyle::Button, FontId::proportional(20.0)),
        (TextStyle::Small, FontId::proportional(14.0)),
    ]
    .into();

    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(16.0, 8.0);
    style.spacing.window_margin = Margin::same(12);

    ctx.set_global_style(style);

    let mut visuals = Visuals::dark();
    visuals.window_fill = menu_backdrop();
    visuals.panel_fill = panel_bg();
    visuals.faint_bg_color = menu_secondary_button();

    visuals.override_text_color = Some(Color32::from_rgba_unmultiplied(235, 240, 250, 255));

    let neon_cyan = accent_solo_cyan();
    let neon_cyan_hover = accent_solo_cyan_hover();
    let panel_border = Color32::from_rgba_unmultiplied(82, 87, 102, 242);

    visuals.widgets.noninteractive.bg_fill = nickname_field_bg();
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, nickname_field_border());
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);

    visuals.widgets.inactive.bg_fill = menu_secondary_button();
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, panel_border);
    visuals.widgets.inactive.fg_stroke =
        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(235, 240, 250, 255));
    visuals.widgets.inactive.corner_radius = CornerRadius::same(6);

    visuals.widgets.hovered.bg_fill = menu_secondary_button_hover();
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, neon_cyan_hover);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(6);

    visuals.widgets.active.bg_fill = neon_cyan;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, neon_cyan);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::BLACK);
    visuals.widgets.active.corner_radius = CornerRadius::same(6);

    ctx.set_visuals(visuals);
}
