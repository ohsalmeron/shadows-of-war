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

/// A cheap vertical gradient (one 2-triangle mesh, `top`→`bottom`). Square corners; used for the
/// objective progress bar's backplate and fill. Mirrors the leader-picker gradient idiom.
fn paint_v_gradient(painter: &egui::Painter, rect: egui::Rect, top: egui::Color32, bottom: egui::Color32) {
    if !rect.is_positive() {
        return;
    }
    let uv = egui::epaint::WHITE_UV;
    let mut mesh = egui::Mesh::default();
    mesh.vertices.push(egui::epaint::Vertex { pos: rect.left_top(), uv, color: top });
    mesh.vertices.push(egui::epaint::Vertex { pos: rect.right_top(), uv, color: top });
    mesh.vertices.push(egui::epaint::Vertex { pos: rect.right_bottom(), uv, color: bottom });
    mesh.vertices.push(egui::epaint::Vertex { pos: rect.left_bottom(), uv, color: bottom });
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

/// Headless Quests tracker anchored top-left: a lone 📜 book button toggles it (mirrors the
/// leaderboard's trophy), and when open a separate, title-less panel lists one tracker row per
/// quest. Returns true if the book button was clicked this frame (caller flips the open state).
/// All emoji via the sow atlas.
pub(super) fn draw_objectives_panel(ctx: &egui::Context, rows: &[ObjRow], open: bool) -> bool {
    use sow_ui::widgets::try_paint_emoji;
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

    let mut toggle = false;
    egui::Area::new(egui::Id::new("tutorial_objectives"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(12.0, if compact { 56.0 } else { 70.0 }))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0; // gap between the button and the rows panel

            // The 📜 toggle, in its own button-hugging frame (no title, no header).
            let btn_prepaint = ui.painter().add(egui::Shape::Noop);
            let btn_frame = egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(
                    sow_ui_kit::theme::margin::COZY,
                    sow_ui_kit::theme::margin::TIGHT,
                ))
                .show(ui, |ui| {
                    if ui
                        .add(sow_ui_kit::widgets::HudEmojiButton::new("📜"))
                        .on_hover_text("Quests")
                        .clicked()
                    {
                        toggle = true;
                    }
                });
            sow_ui_kit::theme::paint_hud_panel_gradient(
                ui,
                btn_prepaint,
                btn_frame.response.rect,
                sow_ui_kit::theme::palette::field_border(),
                radius,
            );

            if !open {
                return;
            }

            // The quest rows, in their own headless panel below the button.
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

                                // Backplate: dark, green-tinted gradient, darker toward the bottom.
                                paint_v_gradient(
                                    painter,
                                    rect,
                                    egui::Color32::from_rgb(13, 28, 20),
                                    egui::Color32::from_rgb(4, 10, 7),
                                );

                                // Fill: bright green up top, deeper green below, plus a top sheen.
                                let fill_w = (rect.width() * frac).round();
                                if fill_w > 1.0 {
                                    let fill =
                                        egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, bar_h));
                                    paint_v_gradient(
                                        painter,
                                        fill,
                                        egui::Color32::from_rgb(96, 240, 150),
                                        egui::Color32::from_rgb(30, 168, 96),
                                    );
                                    painter.rect_filled(
                                        egui::Rect::from_min_size(fill.min, egui::vec2(fill_w, 2.0)),
                                        0,
                                        egui::Color32::from_rgba_unmultiplied(205, 255, 222, 60),
                                    );
                                }

                                // Crisp frame.
                                painter.rect_stroke(
                                    rect,
                                    0,
                                    egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 70, 52)),
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

    toggle
}
