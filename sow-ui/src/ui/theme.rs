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
    let rect = ctx.content_rect();
    let w = rect.width();
    let h = rect.height();
    if w < 480.0 {
        return true;
    }
    w < 600.0 && h < 600.0
}

/// Scale factor for fixed chrome (buttons, gaps, profile bar) on short viewports.
#[inline]
pub fn viewport_scale(ctx: &Context) -> f32 {
    (ctx.content_rect().height() / 720.0).clamp(0.55, 1.0)
}

/// Outer width of the main-menu left rail (content + optional glass frame inset).
#[inline]
pub fn menu_rail_panel_width(available_w: f32, compact: bool) -> f32 {
    if compact {
        return available_w.min(360.0);
    }
    // Fixed ~300px content column; desktop glass frame adds 20px inner margin per side.
    (340.0_f32).min(available_w)
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
            .custom_text_color(text_secondary()),
    )
}

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
        Color32::from_rgba_unmultiplied(12, 12, 14, 50)
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

/// Layout tokens — use these instead of magic numbers in HUD code.
pub mod radius {
    use egui::CornerRadius;

    pub const XS: u8 = 4;
    pub const SM: u8 = 6;
    pub const MD: u8 = 8;
    pub const LG: u8 = 12;

    #[inline]
    pub fn xs() -> CornerRadius {
        CornerRadius::same(XS)
    }
    #[inline]
    pub fn sm() -> CornerRadius {
        CornerRadius::same(SM)
    }
    #[inline]
    pub fn md() -> CornerRadius {
        CornerRadius::same(MD)
    }
    #[inline]
    pub fn lg() -> CornerRadius {
        CornerRadius::same(LG)
    }
    #[inline]
    pub fn tab_top() -> CornerRadius {
        CornerRadius {
            nw: SM,
            ne: SM,
            sw: 0,
            se: 0,
        }
    }
    #[inline]
    pub fn content_bottom() -> CornerRadius {
        CornerRadius {
            nw: 0,
            ne: 0,
            sw: LG,
            se: LG,
        }
    }
}

pub mod stroke {
    pub const HAIRLINE: f32 = 1.0;
    pub const EMPHASIS: f32 = 1.5;
    pub const HEAVY: f32 = 2.0;
}

pub mod margin {
    pub const TIGHT: i8 = 4;
    pub const COZY: i8 = 8;
    pub const REGULAR: i8 = 12;
    pub const LOOSE: i8 = 16;
}

pub mod tab {
    pub const GAP: f32 = 0.0;
    pub const ACCENT_BAR_H: f32 = 2.0;
    pub const BASELINE_H: f32 = 1.0;

    #[inline]
    pub fn height(compact: bool) -> f32 {
        if compact {
            28.0
        } else {
            30.0
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelKind {
    HudOverlay,
    MapControlsRail,
    FloatingCard,
    MenuRail,
}

#[inline]
pub fn panel_frame(kind: PanelKind, compact: bool) -> egui::Frame {
    match kind {
        PanelKind::HudOverlay => {
            // Bottom-left attack ratio and similar compact overlays.
            let (margin_x, margin_y) = if cfg!(target_os = "android") {
                (margin::REGULAR, margin::COZY)
            } else {
                (margin::COZY, margin::TIGHT)
            };
            egui::Frame::NONE
                .fill(Color32::from_black_alpha(150))
                .corner_radius(radius::md())
                .stroke(Stroke::new(stroke::HAIRLINE, nickname_field_border()))
                .inner_margin(Margin::symmetric(margin_x, margin_y))
        }
        PanelKind::MapControlsRail => {
            egui::Frame::NONE
                .fill(Color32::from_black_alpha(150))
                .corner_radius(radius::sm())
                .stroke(Stroke::new(stroke::HAIRLINE, nickname_field_border()))
                .inner_margin(Margin::symmetric(4, margin::TIGHT))
        }
        PanelKind::FloatingCard => standard_panel_frame(compact),
        PanelKind::MenuRail => menu_right_panel_frame(compact),
    }
}

/// Shared fill for active browser tab and connected content card.
#[inline]
pub fn hud_content_fill() -> Color32 {
    palette::field_bg()
}

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
        active_fill: hud_content_fill(),
        baseline: palette::field_border(),
        label_active: palette::text_normal(),
        label_inactive: palette::text_muted(),
    }
}

pub struct CardVisuals {
    pub bg: Color32,
    pub stroke: Stroke,
}

/// Building / action card chrome used across the HUD.
#[inline]
pub fn interact_card(selected: bool, can_afford: bool, hovered: bool, accent: Color32) -> CardVisuals {
    let bg = if selected {
        accent.linear_multiply(0.15)
    } else if hovered {
        palette::field_bg().linear_multiply(1.2)
    } else {
        Color32::from_rgba_unmultiplied(10, 15, 25, 120)
    };
    let stroke = if selected {
        Stroke::new(stroke::EMPHASIS, accent)
    } else if !can_afford {
        Stroke::new(stroke::HAIRLINE, palette::danger())
    } else {
        Stroke::new(stroke::HAIRLINE, palette::field_border().linear_multiply(0.5))
    };
    CardVisuals { bg, stroke }
}

/// Browser-style tab: rounded top, flat bottom, accent stripe when selected.
pub fn draw_tab(
    ui: &mut Ui,
    label: &str,
    selected: bool,
    accent: Color32,
    badge_count: usize,
    tab_w: f32,
    compact: bool,
) -> Response {
    let style = hud_tab_style();
    let tab_h = tab::height(compact);
    let font_size = if compact { 10.0 } else { 11.0 };
    let tab_radius = radius::tab_top();

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

        ui.painter().rect(rect, tab_radius, fill, side_stroke, StrokeKind::Inside);

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

        let label_color = if selected {
            style.label_active
        } else {
            style.label_inactive
        };
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(font_size),
            label_color,
        );

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

/// Browser-style tab with a centered HUD icon instead of a text label.
pub fn draw_icon_tab(
    ui: &mut Ui,
    texture: Option<&TextureHandle>,
    selected: bool,
    accent: Color32,
    badge_count: usize,
    tab_w: f32,
    compact: bool,
) -> Response {
    let style = hud_tab_style();
    let tab_h = tab::height(compact);
    let tab_radius = radius::tab_top();
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

        ui.painter().rect(rect, tab_radius, fill, side_stroke, StrokeKind::Inside);

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

        if let Some(tex) = texture {
            let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(icon_size));
            ui.put(icon_rect, Image::new(tex).fit_to_exact_size(icon_rect.size()));
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
    let baseline = Stroke::new(stroke::HAIRLINE, palette::field_border().linear_multiply(0.85));

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

pub fn apply_theme(ctx: &Context) {
    // JockeyOne only — do not pull in egui bundled default/emoji font blobs.
    let mut fonts = egui::FontDefinitions {
        font_data: Default::default(),
        families: Default::default(),
    };
    fonts.font_data.insert(
        "Default".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(crate::ui_font::UI_FONT_TTF)),
    );
    fonts.families.insert(
        egui::FontFamily::Proportional,
        vec!["Default".to_owned()],
    );
    fonts
        .families
        .insert(egui::FontFamily::Monospace, vec!["Default".to_owned()]);
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
    panel_frame(PanelKind::HudOverlay, false)
}

/// Near-black surface for the in-game leaderboard body.
#[inline]
pub fn leaderboard_panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(6, 8, 12, 235))
        .stroke(egui::Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(255, 255, 255, 20),
        ))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .shadow(egui::Shadow::NONE)
}

/// Dark inset search field used inside the leaderboard panel.
#[inline]
pub fn leaderboard_search_field_bg() -> Color32 {
    Color32::from_rgba_unmultiplied(0, 0, 0, 120)
}

#[inline]
pub fn leaderboard_search_field_border() -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 255, 25)
}

#[inline]
pub fn hud_button_text_size() -> f32 {
    if cfg!(target_os = "android") {
        32.0
    } else {
        18.0
    }
}

/// Red notification badge (circle + count) used on HUD toolbar buttons.
pub fn paint_count_badge(
    painter: &egui::Painter,
    anchor: egui::Pos2,
    count: usize,
    radius: f32,
    font_size: f32,
    cap: Option<usize>,
) {
    if count == 0 {
        return;
    }
    let label = match cap {
        Some(max) if count > max => format!("{max}+"),
        _ => count.to_string(),
    };
    painter.circle_filled(anchor, radius, Color32::from_rgb(239, 68, 68));
    painter.text(
        anchor,
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(font_size),
        Color32::WHITE,
    );
}

#[inline]
pub fn hud_icon_rail_spacing(ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(hud_icon_spacing(), hud_icon_spacing());
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

/// Top vs side chrome in the map editor (shadow strength differs).
#[cfg(feature = "map-editor")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapEditorGlassPanel {
    Top,
    Side,
}

/// Glass panels for the map editor — map stays visible through the center viewport.
#[cfg(feature = "map-editor")]
#[inline]
pub fn map_editor_glass_frame(panel: MapEditorGlassPanel, _compact: bool) -> egui::Frame {
    let (blur, margin, shadow_alpha, offset_y) = match panel {
        MapEditorGlassPanel::Top => (16_u8, Margin::symmetric(24, 18), 15_u8, 6),
        MapEditorGlassPanel::Side => (24_u8, Margin::symmetric(16, 20), 20_u8, 8),
    };
    egui::Frame::new()
        .fill(panel_bg_transparent())
        .stroke(Stroke::new(1.0_f32, palette::field_border()))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(margin)
        .shadow(egui::Shadow {
            blur,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(6, 182, 212, shadow_alpha),
            offset: [0, offset_y],
        })
}

/// Dark translucent rail for main-menu action buttons (no backdrop blur).
#[inline]
pub fn menu_right_panel_frame(compact: bool) -> egui::Frame {
    let fill = Color32::from_rgba_unmultiplied(8, 10, 16, if compact { 175 } else { 155 });
    let margin = if compact { 16 } else { 20 };
    let radius = if compact { CornerRadius::same(10) } else { CornerRadius::same(12) };
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(75, 85, 99, 90),
        ))
        .corner_radius(radius)
        .inner_margin(egui::Margin::same(margin))
        .shadow(egui::Shadow {
            blur: if compact { 16 } else { 20 },
            spread: 0,
            color: Color32::from_black_alpha(80),
            offset: [0, 6],
        })
}

#[inline]
pub fn hud_icon_size() -> f32 {
    if cfg!(target_os = "android") {
        48.0
    } else {
        40.0
    }
}

#[inline]
pub fn hud_icon_spacing() -> f32 {
    margin::TIGHT as f32
}

/// Paint a pre-laid-out galley with premium 7-pass glow (zero layout cost).
///
/// `pos` is the top-left anchor of the galley.
fn paint_premium_glow_galley(
    painter: &egui::Painter,
    pos: egui::Pos2,
    galley: Arc<Galley>,
    base_color: Color32,
    shadow_color: Color32,
) {
    // 1. Dragged-down shadow (2 passes)
    for &dy in &[2.0, 4.0] {
        painter.galley_with_override_text_color(
            pos + egui::vec2(0.0, dy),
            galley.clone(),
            shadow_color,
        );
    }
    // 2. Diagonal outline (4 passes)
    for &(dx, dy) in &[(-1.5, -1.5), (1.5, -1.5), (-1.5, 1.5), (1.5, 1.5)] {
        painter.galley_with_override_text_color(
            pos + egui::vec2(dx, dy),
            galley.clone(),
            shadow_color,
        );
    }
    // 3. Core text (1 pass)
    painter.galley_with_override_text_color(pos, galley, base_color);
}

/// Short soft pixel shadow — opaque at the glyph origin, fading over ~2–3 px.
fn paint_pixel_shadow_galley(
    painter: &egui::Painter,
    pos: Pos2,
    galley: Arc<Galley>,
    base_color: Color32,
) {
    const LAYERS: [(f32, f32, u8); 6] = [
        (0.0, 1.0, 230),
        (1.0, 1.0, 175),
        (0.0, 2.0, 120),
        (1.0, 2.0, 70),
        (2.0, 2.0, 35),
        (1.0, 3.0, 18),
    ];
    for &(dx, dy, alpha) in &LAYERS {
        painter.galley_with_override_text_color(
            pos + Vec2::new(dx, dy),
            galley.clone(),
            Color32::from_black_alpha(alpha),
        );
    }
    painter.galley_with_override_text_color(pos, galley, base_color);
}

/// Leader name in caps with a tight pixel shadow and clear size hierarchy vs body text.
pub fn leader_name_label(ui: &mut Ui, name: &str, size: f32) -> Response {
    leader_caps_line(ui, name, size)
}

/// All-caps white line with the leader picker pixel shadow.
pub fn leader_caps_line(ui: &mut Ui, text: &str, size: f32) -> Response {
    let text = text.to_uppercase();
    let font_id = FontId::proportional(size);
    let galley = ui
        .painter()
        .layout_no_wrap(text, font_id, Color32::WHITE);
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_pixel_shadow_galley(ui.painter(), rect.left_top(), galley, Color32::WHITE);
    }
    response
}

/// Wrapped all-caps block with pixel shadow (ability / perk text).
pub fn leader_caps_paragraph(ui: &mut Ui, text: &str, size: f32, wrap_w: f32) -> Response {
    let text = text.to_uppercase();
    let font_id = FontId::proportional(size);
    let galley = ui
        .painter()
        .layout(text, font_id, Color32::WHITE, wrap_w);
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_pixel_shadow_galley(ui.painter(), rect.left_top(), galley, Color32::WHITE);
    }
    response
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
    let galley = painter.layout_no_wrap(text.to_owned(), font_id, base_color);
    let anchor_pos = anchor_top_left(pos, anchor, galley.size());
    paint_premium_glow_galley(painter, anchor_pos, galley, base_color, shadow_color);
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

/// Draw text with a crisp black outline and heavy bottom drop shadow.
///
/// Convenience wrapper — delegates to [`paint_premium_glow_text`].
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

/// A UI widget that draws text with an outline. Lays out once, paints 7×.
pub fn outlined_label(
    ui: &mut egui::Ui,
    text: &str,
    font_id: FontId,
    color: Color32,
) -> egui::Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font_id, color);
    let (rect, response) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_premium_glow_galley(ui.painter(), rect.left_top(), galley, color, Color32::BLACK);
    }
    response
}

