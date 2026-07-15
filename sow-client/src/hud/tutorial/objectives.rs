//! The detachable Objectives panel (top-left) and its row model. Completed rows fade and
//! slide out after a short hold; the active row shows a live X/Y tracker bar.

#[derive(Clone, Copy, PartialEq)]
pub(super) enum ObjState {
    Done,
    Active,
}

/// One row in the detachable Objectives panel.
pub(super) struct ObjRow {
    pub label: &'static str,
    /// One terse line under the label: what to do + what the X/Y bar counts.
    pub hint: &'static str,
    pub current: u32,
    pub target: u32,
    pub state: ObjState,
    /// 1.0 = full; <1.0 while the completed row fades out before cleanup.
    pub fade: f32,
}

/// A cheap vertical gradient (one 2-triangle mesh or two rounded rects, `top`→`bottom`).
/// Used for the objective progress bar's backplate and fill.
fn paint_v_gradient(
    painter: &egui::Painter,
    rect: egui::Rect,
    top: egui::Color32,
    bottom: egui::Color32,
    radius: egui::CornerRadius,
) {
    if !rect.is_positive() {
        return;
    }
    let mut mesh = egui::Mesh::default();
    let shape = egui::epaint::RectShape::filled(rect, radius, top);
    let tessellator_options = painter.ctx().tessellation_options(|to| *to);
    let mut tessellator = egui::epaint::Tessellator::new(
        painter.ctx().pixels_per_point(),
        tessellator_options,
        [0, 0],
        vec![],
    );
    tessellator.tessellate_rect(&shape, &mut mesh);

    let h = rect.height();
    if h > 0.0 {
        for vertex in &mut mesh.vertices {
            let t = ((vertex.pos.y - rect.min.y) / h).clamp(0.0, 1.0);
            let r = (top.r() as f32 + t * (bottom.r() as f32 - top.r() as f32)) as u8;
            let g = (top.g() as f32 + t * (bottom.g() as f32 - top.g() as f32)) as u8;
            let b = (top.b() as f32 + t * (bottom.b() as f32 - top.b() as f32)) as u8;
            let a = (top.a() as f32 + t * (bottom.a() as f32 - top.a() as f32)) as u8;
            vertex.color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Headless Quests tracker anchored top-left: listed in a title-less panel.
/// Anchored right below the top-left toggle bar.
/// All emoji via the sow atlas.
pub(super) fn draw_objectives_panel(ctx: &egui::Context, rows: &[ObjRow], open: bool) {
    if !open {
        return;
    }
    use sow_ui::widgets::try_paint_emoji;
    let dev = sow_ui_kit::theme::dev_config::DevConfig::get();
    let filler_top = egui::Color32::from_rgba_unmultiplied(
        (dev.obj_filler_top[0] * 255.0) as u8,
        (dev.obj_filler_top[1] * 255.0) as u8,
        (dev.obj_filler_top[2] * 255.0) as u8,
        (dev.obj_filler_top[3] * 255.0) as u8,
    );
    let filler_bottom = egui::Color32::from_rgba_unmultiplied(
        (dev.obj_filler_bottom[0] * 255.0) as u8,
        (dev.obj_filler_bottom[1] * 255.0) as u8,
        (dev.obj_filler_bottom[2] * 255.0) as u8,
        (dev.obj_filler_bottom[3] * 255.0) as u8,
    );
    let backplate_top = egui::Color32::from_rgba_unmultiplied(
        (dev.obj_backplate_top[0] * 255.0) as u8,
        (dev.obj_backplate_top[1] * 255.0) as u8,
        (dev.obj_backplate_top[2] * 255.0) as u8,
        (dev.obj_backplate_top[3] * 255.0) as u8,
    );
    let backplate_bottom = egui::Color32::from_rgba_unmultiplied(
        (dev.obj_backplate_bottom[0] * 255.0) as u8,
        (dev.obj_backplate_bottom[1] * 255.0) as u8,
        (dev.obj_backplate_bottom[2] * 255.0) as u8,
        (dev.obj_backplate_bottom[3] * 255.0) as u8,
    );
    let gold = egui::Color32::from_rgb(255, 200, 90);
    let green = egui::Color32::from_rgb(74, 222, 128);
    let white = egui::Color32::WHITE;
    let compact = ctx.content_rect().width() < 768.0;
    let panel_w = if compact { 160.0_f32 } else { 190.0_f32 };
    let radius = if compact {
        egui::CornerRadius::ZERO
    } else {
        sow_ui_kit::theme::radius::md()
    };

    egui::Area::new(egui::Id::new("tutorial_objectives"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(12.0, 58.0))
        .show(ctx, |ui| {
            ui.style_mut().override_text_style = Some(egui::TextStyle::Small);

            // The quest rows, in their own headless panel.
            let panel_prepaint = ui.painter().add(egui::Shape::Noop);
            let panel_frame = egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(panel_w);
                    // Tight vertical rhythm: label → hint → bar stack closely.
                    ui.spacing_mut().item_spacing.y = 2.0;

                    for (row_i, row) in rows.iter().enumerate() {
                        let accent = match row.state {
                            ObjState::Done => green,
                            ObjState::Active => gold,
                        };
                        if row_i > 0 {
                            ui.add_space(6.0);
                        }
                        // Fade + slide-right as a completed row is cleaned up.
                        ui.scope(|ui| {
                            ui.set_opacity(row.fade);
                            let slide = (1.0 - row.fade) * 14.0;
                            ui.horizontal(|ui| {
                                ui.add_space(slide);
                                ui.label(
                                    egui::RichText::new(row.label)
                                        .size(11.5)
                                        .strong()
                                        .color(accent),
                                );
                                if row.state == ObjState::Done {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let (r, _) = ui.allocate_exact_size(
                                                egui::Vec2::splat(12.0),
                                                egui::Sense::hover(),
                                            );
                                            try_paint_emoji(ui.painter(), "✅", r, green);
                                        },
                                    );
                                }
                            });
                            // The one-line "what to do / what the bar counts" hint. Hidden once
                            // the objective is done (the row is on its way out anyway).
                            if row.state == ObjState::Active && !row.hint.is_empty() {
                                ui.horizontal(|ui| {
                                    ui.add_space(slide);
                                    ui.label(
                                        egui::RichText::new(row.hint)
                                            .size(10.0)
                                            .color(egui::Color32::from_gray(165)),
                                    );
                                });
                            }
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                ui.add_space(slide);
                                let frac_target = if row.target > 0 {
                                    (row.current as f32 / row.target as f32).clamp(0.0, 1.0)
                                } else {
                                    0.0
                                };
                                // Tween the fill toward the live fraction, then settle (stops
                                // repainting once it arrives). Keyed per objective so each bar
                                // animates on its own.
                                let frac = ui.ctx().animate_value_with_time(
                                    egui::Id::new(("obj_bar_fill", row.label)),
                                    frac_target,
                                    0.35,
                                );
                                let bar_h = 16.0_f32;
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(ui.available_width(), bar_h),
                                    egui::Sense::hover(),
                                );
                                let painter = ui.painter();

                                let theme_roundness = if sow_ui_kit::theme::custom_theme_enabled() {
                                    (sow_ui_kit::theme::dev_config::DevConfig::get()
                                        .theme_roundness
                                        .round() as u32)
                                        .min(255) as u8
                                } else {
                                    0
                                };
                                let bar_r_u8 = (theme_roundness / 3).min(8);
                                let cr_bar = egui::CornerRadius::same(bar_r_u8);

                                // Backplate: dark, green-tinted gradient, darker toward the bottom.
                                paint_v_gradient(
                                    painter,
                                    rect,
                                    backplate_top,
                                    backplate_bottom,
                                    cr_bar,
                                );

                                // Fill: bright green up top, deeper green below, plus a top sheen.
                                let fill_w = (rect.width() * frac).round();
                                if fill_w > 1.0 {
                                    let fill = egui::Rect::from_min_size(
                                        rect.min,
                                        egui::vec2(fill_w, bar_h),
                                    );
                                    paint_v_gradient(
                                        painter,
                                        fill,
                                        filler_top,
                                        filler_bottom,
                                        cr_bar,
                                    );
                                    let sheen_cr = egui::CornerRadius {
                                        nw: bar_r_u8,
                                        ne: bar_r_u8,
                                        sw: 0,
                                        se: 0,
                                    };
                                    painter.rect_filled(
                                        egui::Rect::from_min_size(
                                            fill.min,
                                            egui::vec2(fill_w, 2.0),
                                        ),
                                        sheen_cr,
                                        egui::Color32::from_rgba_unmultiplied(205, 255, 222, 60),
                                    );
                                }

                                // Crisp frame.
                                let stroke_color = if sow_ui_kit::theme::custom_theme_enabled() {
                                    let outline_color_raw =
                                        sow_ui_kit::theme::dev_config::DevConfig::get()
                                            .theme_color_outline;
                                    egui::Color32::from_rgba_unmultiplied(
                                        (outline_color_raw[0] * 255.0) as u8,
                                        (outline_color_raw[1] * 255.0) as u8,
                                        (outline_color_raw[2] * 255.0) as u8,
                                        (outline_color_raw[3] * 255.0) as u8,
                                    )
                                } else {
                                    egui::Color32::from_rgb(40, 70, 52)
                                };
                                painter.rect_stroke(
                                    rect,
                                    cr_bar,
                                    egui::Stroke::new(1.0_f32, stroke_color),
                                    egui::StrokeKind::Inside,
                                );

                                // X / Y with a soft shadow so it reads over the gradient.
                                let txt = format!("{} / {}", row.current, row.target);
                                let font = egui::FontId::proportional(10.5);
                                painter.text(
                                    rect.center() + egui::vec2(0.0, 1.0),
                                    egui::Align2::CENTER_CENTER,
                                    txt.clone(),
                                    font.clone(),
                                    egui::Color32::from_black_alpha(160),
                                );
                                painter.text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    txt,
                                    font,
                                    white,
                                );
                            });
                        });
                    }
                });
            sow_ui_kit::theme::paint_hud_panel_gradient(
                ui,
                panel_prepaint,
                panel_frame.response.rect,
                sow_ui_kit::theme::palette::field_border(),
                radius,
            );
        });
}
