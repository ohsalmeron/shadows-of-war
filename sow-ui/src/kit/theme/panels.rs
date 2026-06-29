use super::*;

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
                .fill(palette::surface())
                .corner_radius(radius::md())
                .stroke(Stroke::new(stroke::HAIRLINE, palette::field_border()))
                .inner_margin(Margin::symmetric(margin_x, margin_y))
        }
        PanelKind::MapControlsRail => egui::Frame::NONE
            .fill(palette::surface())
            .corner_radius(radius::sm())
            .stroke(Stroke::new(stroke::HAIRLINE, palette::field_border()))
            .inner_margin(Margin::symmetric(4, margin::TIGHT)),
        PanelKind::FloatingCard => standard_panel_frame(compact),
        PanelKind::MenuRail => menu_right_panel_frame(compact),
    }
}

pub struct CardVisuals {
    pub bg: Color32,
    pub stroke: Stroke,
}

/// Building / action card chrome used across the HUD.
#[inline]
pub fn interact_card(
    selected: bool,
    can_afford: bool,
    hovered: bool,
    accent: Color32,
) -> CardVisuals {
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
        Stroke::new(
            stroke::HAIRLINE,
            palette::field_border().linear_multiply(0.5),
        )
    };
    CardVisuals { bg, stroke }
}
pub fn hud_panel_frame() -> egui::Frame {
    panel_frame(PanelKind::HudOverlay, false)
}

/// Near-black surface for the in-game leaderboard body.
#[inline]
pub fn leaderboard_panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(6, 8, 12, 230))
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
    let stroke = if compact {
        egui::Stroke::NONE
    } else {
        egui::Stroke::new(1.0_f32, palette::neon_cyan_glow())
    };
    let corner = if compact {
        CornerRadius::ZERO
    } else {
        CornerRadius::same(12)
    };
    let margin = if compact { 16 } else { 24 };
    let shadow = if compact {
        egui::Shadow::NONE
    } else {
        egui::Shadow {
            blur: 24,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(6, 182, 212, 30),
            offset: [0, 10],
        }
    };
    egui::Frame::new()
        .fill(palette::surface())
        .stroke(stroke)
        .corner_radius(corner)
        .inner_margin(egui::Margin::same(margin))
        .shadow(shadow)
}

/// A reusable standard pop-up modal dialog template (used for Settings, Terms, Privacy, Credits, and Leaderboards).
pub fn draw_standard_modal<R>(
    ctx: &egui::Context,
    is_open: &mut bool,
    modal_key: &str,
    title: &str,
    close_label: &str,
    reduced_motion: bool,
    content_ui: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    if !*is_open {
        return None;
    }

    let mut result = None;
    let compact = compact_viewport(ctx);
    let screen_rect = ctx.input(|i| i.content_rect());
    let margin = if compact { 0.0 } else { 16.0 };
    
    // Width: take full width on mobile/compact, or cap it on larger desktop screens
    let panel_w = if compact {
        screen_rect.width()
    } else {
        (screen_rect.width() - (margin * 2.0)).min(600.0)
    };
    
    // Height: take full height (minus margin) to ensure maximum layout space under low height
    let panel_h = if compact {
        screen_rect.height()
    } else {
        screen_rect.height() - (margin * 2.0)
    };

    let progress = ctx.animate_bool_with_time(
        egui::Id::new(format!("{modal_key}_animation_progress")),
        *is_open,
        anim_duration(reduced_motion),
    );
    if progress <= 0.01 {
        return None;
    }

    // Semi-transparent scrim backdrop overlay (increased transparency)
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new(format!("{modal_key}_scrim")),
    ))
    .rect_filled(
        screen_rect,
        0.0,
        Color32::from_black_alpha((150.0 * progress) as u8),
    );

    let y_offset = if *is_open {
        let t = progress;
        if t >= 1.0 {
            0.0
        } else {
            -80.0 * (1.0 - t)
        }
    } else {
        0.0
    };

    egui::Window::new(format!("{modal_key}_modal"))
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, y_offset))
        .fixed_size(egui::vec2(panel_w, panel_h))
        .frame(standard_panel_frame(compact))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.set_width(ui.available_width());
                outlined_label(
                    ui,
                    title,
                    egui::FontId::proportional(if compact { 20.0 } else { 24.0 }),
                    Color32::WHITE,
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if modal_close_button(ui).clicked() {
                        *is_open = false;
                    }
                });
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(8.0);

            // Compute remaining height for scroll area
            // Footer: Centered exit close button is 40pt high. With padding/margins, it takes ~60pt.
            let footer_h = 40.0 + 12.0; 
            let available_scroll_h = (ui.available_height() - footer_h - 10.0).max(50.0);

            // Reusable scrollable viewport area
            egui::ScrollArea::vertical()
                .max_height(available_scroll_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    result = Some(content_ui(ui));
                });

            // Footer: Centered exit close button
            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                let close_btn = crate::widgets::ThemeButton::new(close_label)
                    .style(crate::widgets::ThemeButtonStyle::Primary)
                    .min_size(egui::vec2(
                        if compact { ui.available_width() } else { 160.0 },
                        40.0,
                    ));
                if ui.add(close_btn).clicked() {
                    *is_open = false;
                }
            });
        });

    result
}

/// Full-screen sub-page frame (settings, single-player setup, etc.).
///
/// Shared across all screens that replace the main menu with a top/bottom/center layout.
#[inline]
pub fn screen_panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(8, 10, 14))
        .inner_margin(egui::Margin::symmetric(16, 10))
}

/// Top vs side chrome in the map editor (shadow strength differs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapEditorGlassPanel {
    Top,
    Side,
}

/// Glass panels for the map editor — map stays visible through the center viewport.
#[inline]
pub fn map_editor_glass_frame(panel: MapEditorGlassPanel, _compact: bool) -> egui::Frame {
    let (blur, margin, shadow_alpha, offset_y) = match panel {
        MapEditorGlassPanel::Top => (16_u8, Margin::symmetric(24, 18), 15_u8, 6),
        MapEditorGlassPanel::Side => (24_u8, Margin::symmetric(16, 20), 20_u8, 8),
    };
    egui::Frame::new()
        .fill(palette::surface_transparent())
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
    let radius = if compact {
        CornerRadius::same(10)
    } else {
        CornerRadius::same(12)
    };
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

/// Semi-transparent backdrop overlay. Alpha 0.0–1.0.
#[inline]
pub fn paint_scrim(ctx: &egui::Context, layer_id: &'static str, alpha: f32) {
    let screen_rect = ctx.input(|i| i.content_rect());
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new(layer_id),
    ))
    .rect_filled(
        screen_rect,
        0.0,
        Color32::from_black_alpha((180.0 * alpha) as u8),
    );
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

pub fn paint_hud_panel_gradient(
    ui: &mut egui::Ui,
    idx: egui::layers::ShapeIdx,
    rect: egui::Rect,
    border_color: egui::Color32,
    radius: egui::CornerRadius,
) {
    if !rect.is_positive() {
        return;
    }
    let top_color = Color32::from_rgba_unmultiplied(32, 32, 36, 240);
    let bottom_color = Color32::from_rgba_unmultiplied(16, 16, 18, 240);

    let mut mesh = egui::Mesh::default();
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_top(),
        uv: egui::Pos2::ZERO,
        color: top_color,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_top(),
        uv: egui::Pos2::ZERO,
        color: top_color,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_bottom(),
        uv: egui::Pos2::ZERO,
        color: bottom_color,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_bottom(),
        uv: egui::Pos2::ZERO,
        color: bottom_color,
    });
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);

    ui.painter().set(idx, egui::Shape::mesh(mesh));

    // Paint the outline on top
    ui.painter().rect(
        rect,
        radius,
        Color32::TRANSPARENT,
        egui::Stroke::new(1.0_f32, border_color),
        egui::StrokeKind::Inside,
    );
}

