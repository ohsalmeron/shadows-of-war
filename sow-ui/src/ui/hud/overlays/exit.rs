use egui::{vec2, Color32, Context, RichText};
use sow_i18n::Language;

use super::super::state::HudState;
use crate::UiAction;

pub(in crate::ui::hud) fn draw_exit_confirm_overlay(
    ctx: &Context,
    state: &mut HudState,
    lang: Language,
) -> Option<UiAction> {
    let strings = &sow_i18n::get(lang).hud;

    let is_active = state.show_exit_confirm;
    let anim_dur = sow_ui_kit::theme::anim_duration_from_ctx(ctx);
    let anim = crate::ui::animation::panel_in_out_anim(
        ctx,
        egui::Id::new("exit_confirm_panel_animation"),
        is_active,
        anim_dur,
        crate::ui::animation::PANEL_Y_SLIDE,
        crate::ui::animation::SlideDir::Down,
    );

    if anim.progress <= 0.01 {
        return None;
    }

    let alpha = anim.progress;
    let y_offset = anim.offset;
    let screen_rect = ctx.content_rect();
    let compact = screen_rect.width() < 768.0 || screen_rect.width() < screen_rect.height() * 1.25;

    sow_ui_kit::theme::paint_scrim(ctx, "exit_confirm_overlay_bg", alpha);

    let window = egui::Window::new("exit_confirm_modal")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .order(egui::Order::Foreground);

    let panel_w = if compact {
        (screen_rect.width() - 32.0).min(450.0)
    } else {
        480.0
    };

    let window = window.fixed_size(vec2(panel_w, 0.0)).anchor(
        egui::Align2::CENTER_CENTER,
        vec2(0.0, if compact { y_offset } else { -20.0 + y_offset }),
    );

    let border_color = sow_ui_kit::theme::palette::danger().linear_multiply(alpha);
    let mut exit_clicked = false;

    window
        .frame(
            sow_ui_kit::theme::standard_panel_frame(compact)
                .fill(sow_ui_kit::theme::palette::surface().linear_multiply(alpha))
                .stroke(egui::Stroke::new(2.0f32 * anim.scale, border_color))
                .inner_margin(if compact { 16.0 } else { 24.0 }),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                crate::ui::theme::outlined_label(
                    ui,
                    &strings.confirm_exit_title,
                    egui::FontId::proportional(if compact { 22.0 } else { 26.0 }),
                    border_color,
                );

                ui.add_space(if compact { 12.0 } else { 16.0 });

                ui.label(
                    RichText::new(&strings.confirm_exit_body)
                        .size(if compact { 14.0 } else { 15.0 })
                        .color(Color32::WHITE.linear_multiply(alpha)),
                );

                ui.add_space(if compact { 20.0 } else { 24.0 });

                let btn_w = if compact {
                    (ui.available_width() - 12.0) / 2.0
                } else {
                    160.0
                };
                let btn_h = if compact { 40.0 } else { 44.0 };

                ui.horizontal(|ui| {
                    let spacing = if compact { 12.0 } else { 16.0 };
                    ui.spacing_mut().item_spacing.x = spacing;

                    let total_width = btn_w * 2.0 + spacing;
                    let available = ui.available_width();
                    if available > total_width {
                        ui.add_space((available - total_width) / 2.0);
                    }

                    if ui
                        .add(
                            crate::widgets::ThemeButton::new(&strings.confirm_exit_no)
                                .style(crate::widgets::ThemeButtonStyle::Tertiary)
                                .min_size(vec2(btn_w, btn_h))
                                .text_size(if compact { 13.0 } else { 15.0 }),
                        )
                        .clicked()
                    {
                        state.show_exit_confirm = false;
                    }

                    if ui
                        .add(
                            crate::widgets::ThemeButton::new(&strings.confirm_exit_yes)
                                .style(crate::widgets::ThemeButtonStyle::Danger)
                                .min_size(vec2(btn_w, btn_h))
                                .text_size(if compact { 13.0 } else { 15.0 }),
                        )
                        .clicked()
                    {
                        state.show_exit_confirm = false;
                        exit_clicked = true;
                    }
                });
            });
        });

    if exit_clicked {
        Some(UiAction::LeaveLobby)
    } else {
        None
    }
}
