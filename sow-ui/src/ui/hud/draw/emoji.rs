use egui::{pos2, vec2, Align2, Color32, RichText};
use sow_i18n::Language;

use super::super::state::{
    hud_map_controls_anchor_offset, HudState, HUD_MAP_CONTROLS_MOBILE_FALLBACK_CLEARANCE,
};

pub(in crate::ui::hud) fn draw_emoji_panel(
    ui: &mut egui::Ui,
    state: &mut HudState,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    lang: Language,
    compact: bool,
    anim: f32,
    anim_hover: f32,
) {
    let is_emoji_active = state.show_emoji_panel;
    let emoji_progress = ui.ctx().animate_bool_with_time(
        egui::Id::new("emoji_panel_animation"),
        is_emoji_active,
        anim,
    );
    if emoji_progress <= 0.01 {
        return;
    }

    let anim_scale = if is_emoji_active {
        let t = emoji_progress;
        if t >= 1.0 {
            1.0
        } else {
            1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
        }
    } else {
        emoji_progress
    };

    let screen_rect = ui.ctx().content_rect();
    let emojis = &[
        "😀", "😎", "😏", "😂", "🤣", "😋", "😉", "😜", "😍", "🥰", "🥳", "🥺", "😇", "🤩", "👍",
        "❤️", "😮", "🤔", "🧐", "🙄", "🤯", "🤡", "💩", "🤫", "😠", "😡", "🤬", "😤", "🥵", "🥶",
        "🤢", "🤮", "⚔️", "🛡️", "🏹", "💣", "💥", "💀", "👑", "💪", "🔥", "👀", "🏳️", "🤝", "💔",
        "🔌", "⭐", "🐺",
    ];

    if compact {
        ui.ctx()
            .layer_painter(egui::LayerId::new(
                egui::Order::Background,
                egui::Id::new("emoji_dim_bg"),
            ))
            .rect_filled(
                screen_rect,
                0.0,
                Color32::from_black_alpha((150.0 * emoji_progress) as u8),
            );
    }

    let mut area =
        egui::Area::new(egui::Id::new("floating_emoji_panel")).order(egui::Order::Foreground);

    let y_offset = 120.0 * (1.0 - anim_scale);
    if compact {
        let pos = screen_rect.center() + vec2(0.0, y_offset);
        area = area.pivot(Align2::CENTER_CENTER).fixed_pos(pos);
    } else if let Some(pos) = state.emoji_panel_pos {
        let pos = pos + vec2(0.0, y_offset);
        area = area.pivot(Align2::CENTER_CENTER).fixed_pos(pos);
    } else {
        area = area.anchor(
            Align2::RIGHT_BOTTOM,
            vec2(
                -64.0,
                hud_map_controls_anchor_offset(ui.ctx(), false, state.safe_area_bottom).y
                    + y_offset,
            ),
        );
    }

    let border_glow = Color32::from_rgb(251, 191, 36).linear_multiply(emoji_progress);

    area.show(ui.ctx(), |ui| {
        let frame_res = egui::Frame::window(&ui.ctx().global_style())
            .fill(crate::ui::theme::palette::surface().linear_multiply(emoji_progress))
            .stroke(egui::Stroke::new(1.8_f32 * anim_scale, border_glow))
            .shadow(egui::Shadow {
                blur: if compact { 12 } else { 16 },
                spread: 0,
                color: border_glow.linear_multiply(0.25 * emoji_progress),
                offset: [0, 0],
            })
            .inner_margin(egui::Margin::same(
                ((if compact { 10.0 } else { 12.0 }) * anim_scale) as i8,
            ))
            .corner_radius(((if compact { 16.0 } else { 12.0 }) * anim_scale) as u8)
            .show(ui, |ui| {
                let cols = 8;
                let btn_size = (if compact { 44.0 } else { 48.0 }) * anim_scale;
                let spacing = (if compact { 2.0 } else { 3.0 }) * anim_scale;
                let grid_width = cols as f32 * btn_size + (cols as f32 - 1.0) * spacing;

                ui.vertical(|ui| {
                    ui.set_width(grid_width);
                    ui.spacing_mut().item_spacing.y = 0.0;

                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(&sow_i18n::get(lang).hud.emoji_panel_title)
                                .strong()
                                .size(13.0 * anim_scale)
                                .color(border_glow),
                        );
                    });
                    ui.add_space(8.0 * anim_scale);

                    let mut emoji_idx = 0;
                    for chunk in emojis.chunks(cols) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = spacing;
                            for &emoji in chunk {
                                let (rect, resp) = ui.allocate_exact_size(
                                    vec2(btn_size, btn_size),
                                    egui::Sense::click(),
                                );
                                let is_hovered = resp.hovered();

                                let scale_id = ui.make_persistent_id(("emoji_scale", emoji_idx));
                                let hover_t = ui
                                    .ctx()
                                    .animate_bool_with_time(scale_id, is_hovered, anim_hover);

                                let spring_t = if hover_t > 0.0 {
                                    let c1 = 1.70158;
                                    let c3 = c1 + 1.0;
                                    let xm1 = hover_t - 1.0;
                                    1.0 + c3 * xm1 * xm1 * xm1 + c1 * xm1 * xm1
                                } else {
                                    0.0
                                };

                                let bg_color = if is_hovered {
                                    crate::ui::theme::palette::button_hovered()
                                        .linear_multiply((0.6 + 0.3 * hover_t) * emoji_progress)
                                } else {
                                    crate::ui::theme::palette::field_bg()
                                        .linear_multiply(0.4 * emoji_progress)
                                };
                                let stroke_color = border_glow.linear_multiply(0.3 + 0.7 * hover_t);
                                let active_stroke = egui::Stroke::new(
                                    (1.0_f32 + 1.0_f32 * hover_t) * anim_scale,
                                    stroke_color,
                                );

                                let active_rect = rect.expand(2.0 * spring_t * anim_scale);

                                ui.painter().rect(
                                    active_rect,
                                    8.0 * anim_scale,
                                    bg_color,
                                    active_stroke,
                                    egui::StrokeKind::Inside,
                                );

                                let paint_size = btn_size * (0.78 + 0.1 * spring_t);
                                let emoji_rect = egui::Rect::from_center_size(
                                    active_rect.center(),
                                    egui::vec2(paint_size, paint_size),
                                );
                                let tint = Color32::WHITE.linear_multiply(emoji_progress);
                                if !crate::widgets::try_paint_emoji(
                                    ui.painter(),
                                    emoji,
                                    emoji_rect,
                                    tint,
                                ) {
                                    ui.painter().text(
                                        active_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        emoji,
                                        egui::FontId::proportional(paint_size * 0.9),
                                        tint,
                                    );
                                }

                                emoji_idx += 1;

                                if resp.clicked() {
                                    let intent = sow_core::protocol::GameplayIntent::ExpressEmoji {
                                        emoji: emoji.to_owned(),
                                        pinned: state.pin_emoji,
                                    };
                                    cancel_intents.push(intent);
                                    if !state.pin_emoji {
                                        state.show_emoji_panel = false;
                                    }
                                }
                            }
                        });
                        ui.add_space(spacing);
                    }

                    ui.add_space(8.0 * anim_scale);

                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0 * anim_scale;

                                let text_color = if state.pin_emoji {
                                    crate::ui::theme::palette::neon_gold()
                                        .linear_multiply(emoji_progress)
                                } else {
                                    crate::ui::theme::palette::text_muted()
                                        .linear_multiply(emoji_progress)
                                };

                                let label_resp = ui.label(
                                    RichText::new("PIN")
                                        .size(13.0 * anim_scale)
                                        .strong()
                                        .color(text_color),
                                );

                                let label_click = ui.interact(
                                    label_resp.rect,
                                    ui.make_persistent_id("pin_label_click"),
                                    egui::Sense::click(),
                                );
                                if label_click.clicked() {
                                    state.pin_emoji = !state.pin_emoji;
                                }

                                let box_size = 28.0;
                                let (rect, resp) = ui.allocate_exact_size(
                                    vec2(box_size, box_size),
                                    egui::Sense::click(),
                                );

                                let is_hovered = resp.hovered();
                                if resp.clicked() {
                                    state.pin_emoji = !state.pin_emoji;
                                }

                                let bg_color = if state.pin_emoji {
                                    crate::ui::theme::palette::neon_gold().linear_multiply(0.2)
                                } else {
                                    crate::ui::theme::palette::field_bg()
                                };

                                let stroke_color = if state.pin_emoji {
                                    crate::ui::theme::palette::neon_gold()
                                } else if is_hovered {
                                    crate::ui::theme::palette::neon_cyan()
                                } else {
                                    crate::ui::theme::palette::field_border()
                                };

                                ui.painter().rect(
                                    rect,
                                    4.0,
                                    bg_color,
                                    egui::Stroke::new(2.0_f32, stroke_color),
                                    egui::StrokeKind::Inside,
                                );

                                if state.pin_emoji {
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "✓",
                                        egui::FontId::proportional(22.0),
                                        crate::ui::theme::palette::neon_gold(),
                                    );
                                }
                            });
                        });
                    });
                });
            });
        let response_rect = frame_res.response.rect;
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("emoji_panel_rect"), response_rect));
    });

    if !state.emoji_panel_just_opened && ui.ctx().input(|i| i.pointer.any_pressed()) {
        if let Some(pos) = ui
            .ctx()
            .input(|i| i.pointer.press_origin().or(i.pointer.interact_pos()))
        {
            let mut is_outside = true;

            if let Some(rect) = ui
                .ctx()
                .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("emoji_panel_rect")))
            {
                if rect.contains(pos) {
                    is_outside = false;
                }
            }

            if state.emoji_panel_pos.is_none() {
                let screen_size = ui.ctx().content_rect();
                let hud_rect = ui
                    .ctx()
                    .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("hud_bottom_panel_rect")))
                    .unwrap_or_else(|| {
                        egui::Rect::from_min_max(
                            pos2(
                                screen_size.right() - 510.0,
                                screen_size.bottom()
                                    - HUD_MAP_CONTROLS_MOBILE_FALLBACK_CLEARANCE
                                    - state.safe_area_bottom,
                            ),
                            pos2(screen_size.right(), screen_size.bottom()),
                        )
                    });
                if hud_rect.contains(pos) {
                    is_outside = false;
                }
            }

            if is_outside && !state.pin_emoji {
                state.show_emoji_panel = false;
            }
        }
    }

    state.emoji_panel_just_opened = false;
}
