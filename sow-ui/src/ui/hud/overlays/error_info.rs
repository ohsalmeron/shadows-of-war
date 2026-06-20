use egui::{vec2, Color32, Context};
use sow_i18n::Language;
use web_time::{Duration, Instant};

use super::super::state::HudState;

pub(in crate::ui::hud) fn draw_error_overlay(ctx: &Context, state: &mut HudState, _lang: Language) {
    let is_active = state.show_error.is_some();
    let anim = sow_ui_kit::theme::anim_duration_from_ctx(ctx);
    let progress =
        ctx.animate_bool_with_time(egui::Id::new("error_toast_animation"), is_active, anim);

    if progress <= 0.01 && !is_active {
        state.last_error_message = None;
        return;
    }

    if let Some(err_msg) = state.show_error.clone() {
        let now = Instant::now();
        let display_duration = Duration::from_millis(2500);

        let reset = state.last_error_message.as_ref() != Some(&err_msg);

        if reset {
            state.last_error_message = Some(err_msg.clone());
            state.error_display_timer = Some(now);
        }

        let start_time = state.error_display_timer.unwrap_or(now);
        let elapsed = now.duration_since(start_time);

        if elapsed >= display_duration {
            state.show_error = None;
            state.error_display_timer = None;
        }
    }

    let err_msg = match &state.last_error_message {
        Some(msg) => msg.clone(),
        None => return,
    };

    // Disney overshoot curve (pop-in pop-out spring animation)
    let anim_scale = if is_active {
        let t = progress;
        if t >= 1.0 {
            1.0
        } else {
            1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
        }
    } else {
        progress
    };

    let alpha = progress;
    let bg_color = Color32::from_rgba_unmultiplied(15, 23, 42, (180.0 * alpha) as u8);
    let border_color = sow_ui_kit::theme::palette::danger().linear_multiply(alpha);
    let text_color = Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8);

    let target_y = 80.0 + state.safe_area_top;
    // Slide down from above the screen (-120px) to target with a beautiful overshoot bounce
    let current_y = target_y - 120.0 * (1.0 - anim_scale);

    egui::Area::new(egui::Id::new("error_toast_area"))
        .anchor(egui::Align2::CENTER_TOP, vec2(0.0, current_y))
        .order(egui::Order::Tooltip)
        .show(ctx, |ui| {
            let frame = egui::Frame::new()
                .fill(bg_color)
                .stroke(egui::Stroke::new(1.0_f32, border_color))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(16, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        crate::widgets::outlined_emoji_label(
                            ui,
                            "⚠️",
                            egui::FontId::proportional(12.0),
                            border_color,
                        );
                        ui.add_space(6.0);
                        crate::widgets::outlined_emoji_label(
                            ui,
                            &err_msg,
                            egui::FontId::proportional(12.0),
                            text_color,
                        );
                    });
                });
            let response = ui.interact(
                frame.response.rect,
                ui.make_persistent_id("error_toast_click"),
                egui::Sense::click(),
            );
            if response.clicked() {
                state.show_error = None;
                state.error_display_timer = None;
            }
        });

    // Request repaint so the fade-out/pop-out animation runs smoothly
    ctx.request_repaint();
}

pub(in crate::ui::hud) fn draw_info_overlay(ctx: &Context, state: &mut HudState, _lang: Language) {
    let is_active = state.show_info.is_some();
    let anim = sow_ui_kit::theme::anim_duration_from_ctx(ctx);
    let progress =
        ctx.animate_bool_with_time(egui::Id::new("info_toast_animation"), is_active, anim);

    if progress <= 0.01 && !is_active {
        state.last_info_message = None;
        return;
    }

    if let Some(info_msg) = state.show_info.clone() {
        let now = Instant::now();
        let display_duration = Duration::from_millis(2500);

        let reset = state.last_info_message.as_ref() != Some(&info_msg);

        if reset {
            state.last_info_message = Some(info_msg.clone());
            state.info_display_timer = Some(now);
        }

        let start_time = state.info_display_timer.unwrap_or(now);
        let elapsed = now.duration_since(start_time);

        if elapsed >= display_duration {
            state.show_info = None;
            state.info_display_timer = None;
        }
    }

    let info_msg = match &state.last_info_message {
        Some(msg) => msg.clone(),
        None => return,
    };

    // Disney overshoot curve (pop-in pop-out spring animation)
    let anim_scale = if is_active {
        let t = progress;
        if t >= 1.0 {
            1.0
        } else {
            1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
        }
    } else {
        progress
    };

    let alpha = progress;
    let bg_color = Color32::from_rgba_unmultiplied(15, 23, 42, (180.0 * alpha) as u8);
    let border_color = sow_ui_kit::theme::palette::neon_cyan().linear_multiply(alpha);
    let text_color = Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8);

    let target_y = 80.0 + state.safe_area_top;
    // Slide down from above the screen (-120px) to target with a beautiful overshoot bounce
    let current_y = target_y - 120.0 * (1.0 - anim_scale);

    egui::Area::new(egui::Id::new("info_toast_area"))
        .anchor(egui::Align2::CENTER_TOP, vec2(0.0, current_y))
        .order(egui::Order::Tooltip)
        .show(ctx, |ui| {
            let frame = egui::Frame::new()
                .fill(bg_color)
                .stroke(egui::Stroke::new(1.0_f32, border_color))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(16, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        crate::widgets::outlined_emoji_label(
                            ui,
                            "🤝",
                            egui::FontId::proportional(12.0),
                            border_color,
                        );
                        ui.add_space(6.0);
                        crate::widgets::outlined_emoji_label(
                            ui,
                            &info_msg,
                            egui::FontId::proportional(12.0),
                            text_color,
                        );
                    });
                });
            let response = ui.interact(
                frame.response.rect,
                ui.make_persistent_id("info_toast_click"),
                egui::Sense::click(),
            );
            if response.clicked() {
                state.show_info = None;
                state.info_display_timer = None;
            }
        });

    // Request repaint so the fade-out/pop-out animation runs smoothly
    ctx.request_repaint();
}
