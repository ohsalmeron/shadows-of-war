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
    pub current: u32,
    pub target: u32,
    pub state: ObjState,
    /// 1.0 = full; <1.0 while the completed row fades out before cleanup.
    pub fade: f32,
}

/// Detachable Objectives panel anchored top-left: a 📜 scroll icon + "OBJECTIVES" header
/// (click to collapse/expand) and one tracker row per objective. Returns true if the header
/// was clicked this frame (caller flips the open state). All emoji via the sow atlas.
pub(super) fn draw_objectives_panel(ctx: &egui::Context, rows: &[ObjRow], open: bool) -> bool {
    use sow_ui::widgets::try_paint_emoji;
    let gold = egui::Color32::from_rgb(255, 200, 90);
    let green = egui::Color32::from_rgb(74, 222, 128);
    let muted = egui::Color32::from_gray(150);
    let white = egui::Color32::WHITE;
    let compact = ctx.content_rect().width() < 768.0;
    let panel_w = if compact { 160.0_f32 } else { 190.0_f32 };

    let mut toggle = false;
    egui::Area::new(egui::Id::new("tutorial_objectives"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(12.0, if compact { 56.0 } else { 70.0 }))
        .show(ctx, |ui| {
            sow_ui_kit::theme::hud_panel_frame().show(ui, |ui| {
                ui.set_width(panel_w);

                // Header: scroll icon + title + caret. The whole row toggles the panel.
                let header = ui.horizontal(|ui| {
                    let (icon_r, _) =
                        ui.allocate_exact_size(egui::Vec2::splat(18.0), egui::Sense::hover());
                    try_paint_emoji(ui.painter(), "📜", icon_r, gold);
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("OBJECTIVES")
                            .size(13.0)
                            .strong()
                            .color(white),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (r, _) = ui.allocate_exact_size(
                            egui::Vec2::splat(12.0),
                            egui::Sense::hover(),
                        );
                        try_paint_emoji(ui.painter(), if open { "➖" } else { "➕" }, r, muted);
                    });
                });
                let resp = ui.interact(
                    header.response.rect,
                    egui::Id::new("tutorial_objectives_toggle"),
                    egui::Sense::click(),
                );
                if resp.clicked() {
                    toggle = true;
                }
                resp.on_hover_cursor(egui::CursorIcon::PointingHand);

                if open {
                    for row in rows {
                        let accent = match row.state {
                            ObjState::Done => green,
                            ObjState::Active => gold,
                        };
                        ui.add_space(4.0);
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
                            ui.add_space(1.0);
                            ui.horizontal(|ui| {
                                ui.add_space(slide);
                                let frac = if row.target > 0 {
                                    (row.current as f32 / row.target as f32).clamp(0.0, 1.0)
                                } else {
                                    0.0
                                };
                                let bar_h = 12.0_f32;
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(ui.available_width(), bar_h),
                                    egui::Sense::hover(),
                                );
                                let painter = ui.painter();
                                painter.rect_filled(
                                    rect,
                                    egui::CornerRadius::same(3),
                                    egui::Color32::from_black_alpha(120),
                                );
                                let fill_w = rect.width() * frac;
                                if fill_w > 0.5 {
                                    painter.rect_filled(
                                        egui::Rect::from_min_size(
                                            rect.min,
                                            egui::vec2(fill_w, bar_h),
                                        ),
                                        egui::CornerRadius::same(3),
                                        accent.gamma_multiply(0.85),
                                    );
                                }
                                painter.text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    format!("{} / {}", row.current, row.target),
                                    egui::FontId::proportional(10.0),
                                    white,
                                );
                            });
                        });
                    }
                }
            });
        });

    toggle
}
