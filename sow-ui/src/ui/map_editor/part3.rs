fn paint_chip(ui: &mut Ui, label: &str, selected: bool) -> egui::Response {
    let accent = sow_ui_kit::theme::palette::neon_cyan();
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
    let hovered = response.hovered();
    let visuals = sow_ui_kit::theme::interact_card(selected, true, hovered, accent);

    if ui.is_rect_visible(rect) {
        ui.painter().rect(
            rect,
            egui::CornerRadius::same(6),
            visuals.bg,
            visuals.stroke,
            egui::StrokeKind::Inside,
        );
        sow_ui_kit::theme::paint_premium_glow_text(
            ui.painter(),
            rect.left_center() + Vec2::new(10.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(13.0),
            if selected {
                accent
            } else {
                sow_ui_kit::theme::palette::text_normal()
            },
            Color32::BLACK,
        );
    }

    response
}

fn tile_center_screen(viewport: MapEditorViewport, tx: f32, ty: f32) -> egui::Pos2 {
    egui::Pos2::new(
        tx * viewport.zoom + viewport.camera_x,
        ty * viewport.zoom + viewport.camera_y,
    )
}

fn draw_osm_picker_canvas(ui: &mut Ui, view: &OsmPickerView, state: &mut MapEditorUiState) {
    let rect = ui.max_rect();
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(30, 30, 30));
    for tile in &view.tiles {
        if rect.intersects(tile.rect) {
            painter.image(
                tile.texture,
                tile.rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }

    if response.drag_started_by(egui::PointerButton::Primary)
        && let Some(pos) = response.interact_pointer_pos() {
            state.osm_drag_anchor = Some(pos);
        }
    if response.dragged_by(egui::PointerButton::Primary)
        && let (Some(start), Some(current)) =
            (state.osm_drag_anchor, response.interact_pointer_pos())
        {
            state.osm_selection_screen = Some(egui::Rect::from_two_pos(start, current));
        }
    if response.drag_stopped() {
        state.osm_drag_anchor = None;
    }

    let sel = state.osm_selection_screen.or(view.selection_screen_rect);
    if let Some(sel) = sel {
        painter.rect_stroke(
            sel,
            0.0,
            Stroke::new(2.0_f32, sow_ui_kit::theme::palette::neon_cyan()),
            egui::StrokeKind::Outside,
        );
        painter.rect_filled(sel, 0.0, Color32::from_rgba_unmultiplied(6, 182, 212, 40));
    }
}

fn draw_viewport_overlay(ui: &mut Ui, viewport: MapEditorViewport, state: &MapEditorUiState) {
    if !ui.is_rect_visible(ui.max_rect()) {
        return;
    }
    let painter = ui.painter();
    let accent = sow_ui_kit::theme::palette::neon_cyan();

    for spawn in &state.spawns {
        let center = tile_center_screen(viewport, spawn.x as f32 + 0.5, spawn.y as f32 + 0.5);
        if !ui.clip_rect().contains(center) {
            continue;
        }
        let radius = (viewport.zoom * 5.0).clamp(4.0, 24.0);
        painter.circle_filled(
            center,
            radius,
            Color32::from_rgba_unmultiplied(6, 182, 212, 60),
        );
        painter.circle_stroke(center, radius, Stroke::new(2.0_f32, accent));
        painter.text(
            center,
            Align2::CENTER_CENTER,
            &spawn.flag,
            egui::FontId::proportional((radius * 1.2).clamp(12.0, 22.0)),
            Color32::WHITE,
        );
        sow_ui_kit::theme::paint_premium_glow_text(
            painter,
            center + Vec2::new(0.0, radius + 6.0),
            Align2::CENTER_TOP,
            &spawn.name,
            egui::FontId::proportional(11.0),
            Color32::WHITE,
            Color32::BLACK,
        );
    }

    let map_rect = ui.max_rect();
    if map_rect.contains(egui::pos2(viewport.pointer_x, viewport.pointer_y)) {
        let world_x = (viewport.pointer_x - viewport.camera_x) / viewport.zoom;
        let world_y = (viewport.pointer_y - viewport.camera_y) / viewport.zoom;
        let cx = world_x.round() + 0.5;
        let cy = world_y.round() + 0.5;
        let center = tile_center_screen(viewport, cx, cy);
        let brush_r = state.brush_size as f32 * viewport.zoom;
        painter.circle_stroke(
            center,
            brush_r,
            Stroke::new(1.5_f32, accent.linear_multiply(0.85)),
        );
        painter.circle_filled(center, 3.0_f32, accent.linear_multiply(0.9));
    }
}

pub(super) fn draw_confirm_dialog(
    ctx: &Context,
    msg: (&str, &str),
    labels: (&str, &str),
    open: &mut bool,
    compact: bool,
    on_yes: impl FnOnce() -> MapEditorAction,
    action: &mut MapEditorAction,
) {
    let (title, body) = msg;
    let (yes_label, no_label) = labels;
    if !*open {
        return;
    }
    let mut still_open = true;
    egui::Window::new(title)
        .open(&mut still_open)
        .resizable(false)
        .collapsible(false)
        .frame(sow_ui_kit::theme::standard_panel_frame(compact))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label(RichText::new(body).size(14.0));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui
                    .add(
                        ThemeButton::new(yes_label)
                            .style(ThemeButtonStyle::Primary)
                            .text_size(14.0),
                    )
                    .clicked()
                {
                    *action = on_yes();
                    *open = false;
                }
                ui.add_space(8.0);
                if ui
                    .add(
                        ThemeButton::new(no_label)
                            .style(ThemeButtonStyle::Tertiary)
                            .text_size(14.0),
                    )
                    .clicked()
                {
                    *open = false;
                }
            });
        });
    if !still_open {
        *open = false;
    }
}

fn draw_toast(ctx: &Context, state: &mut MapEditorUiState) {
    const DISPLAY_SECS: f32 = 2.5;

    if let Some(start) = state.toast_started
        && state.toast_message.is_some() && start.elapsed().as_secs_f32() >= DISPLAY_SECS {
            state.toast_message = None;
            state.toast_started = None;
        }

    let is_active = state.toast_message.is_some();
    let anim = sow_ui_kit::theme::anim_duration_from_ctx(ctx);
    let progress = ctx.animate_bool_with_time(egui::Id::new("map_editor_toast"), is_active, anim);

    if progress <= 0.01 && !is_active {
        state.toast_last_message = None;
        return;
    }

    if progress > 0.0 && progress < 1.0 {
        ctx.request_repaint();
    }

    let message = match state.toast_last_message.clone() {
        Some(msg) => msg,
        None => return,
    };

    let alpha = progress;
    let accent = if state.toast_is_error {
        sow_ui_kit::theme::palette::danger()
    } else {
        sow_ui_kit::theme::palette::neon_cyan()
    };
    let bg_color = Color32::from_rgba_unmultiplied(15, 23, 42, (180.0 * alpha) as u8);
    let border_color = accent.linear_multiply(alpha);
    let text_color = Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8);

    egui::Area::new(egui::Id::new("map_editor_toast_area"))
        .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -50.0))
        .order(Order::Tooltip)
        .show(ctx, |ui| {
            let frame_response = egui::Frame::new()
                .fill(bg_color)
                .stroke(Stroke::new(1.0_f32, border_color))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(16, 8))
                .show(ui, |ui| {
                    ui.label(RichText::new(message).color(text_color).size(13.0).strong());
                });
            if frame_response.response.clicked() {
                state.toast_message = None;
                state.toast_started = None;
            }
        });
}
