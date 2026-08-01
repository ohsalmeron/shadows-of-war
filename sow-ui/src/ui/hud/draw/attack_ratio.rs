//! The attack-ratio (⚔) vertical slider, decoupled from the bottom panel into its own bottom-left
//! rail — the mirror of the bottom-right map-controls rail: same `MapControlsRail` frame, same
//! width. Everything (the %, the troop count, the track) is laid out *within* that width; only the
//! draggable knob is allowed to spill past it.

use crate::UiAction;
use egui::{Align2, Color32, CornerRadius, Rect, Stroke, pos2, vec2};

use super::super::state::{HudState, hud_map_controls_anchor_offset};

const KNOB_D: f32 = 28.0;
const TRACK_W: f32 = 10.0;
const TRACK_H: f32 = 80.0;
const RATIO_MIN: f32 = 0.01;
const RATIO_MAX: f32 = 1.0;

#[inline]
fn ratio_from_track_y(track: Rect, y: f32) -> f32 {
    let t = 1.0 - ((y - track.top()) / track.height()).clamp(0.0, 1.0);
    RATIO_MIN + t * (RATIO_MAX - RATIO_MIN)
}

#[inline]
fn knob_y(track: Rect, ratio: f32) -> f32 {
    let t = ((ratio - RATIO_MIN) / (RATIO_MAX - RATIO_MIN)).clamp(0.0, 1.0);
    track.bottom() - t * track.height()
}

fn paint_centered_label(
    painter: &egui::Painter,
    rect: Rect,
    text: &str,
    font_size: f32,
    color: Color32,
) {
    let font_id = egui::FontId::proportional(font_size);
    let galley = painter.layout_no_wrap(text.to_owned(), font_id, color);
    let pos = rect.center() - galley.size() * 0.5;
    sow_ui_kit::theme::paint_premium_glow_galley(painter, pos, galley, color, Color32::BLACK);
}

pub(in crate::ui::hud) fn draw_attack_ratio_rail(
    ui: &mut egui::Ui,
    state: &HudState,
    compact: bool,
    action: &mut Option<UiAction>,
) {
    // Nothing to commit during the deployment phase — hide it, like the build controls.
    if state.spawn_timer_secs.is_some() {
        return;
    }

    // Mirror the map-controls rail into the opposite (left) corner: same vertical offset, flipped x.
    let mut offset = hud_map_controls_anchor_offset(ui.ctx(), compact, state.safe_area_bottom);
    offset.x = -offset.x;

    egui::Area::new(egui::Id::new("hud_attack_ratio"))
        .anchor(Align2::LEFT_BOTTOM, offset)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            let btn_w = if cfg!(target_os = "android") {
                46.0
            } else {
                30.0
            };
            let rail_pad_x = 4.0;
            let rail_w = btn_w + rail_pad_x * 2.0;
            ui.set_width(rail_w);
            ui.set_max_width(rail_w);

            let prepaint_idx = ui.painter().add(egui::Shape::Noop);
            let frame_res = egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(4, sow_ui_kit::theme::margin::TIGHT))
                .show(ui, |ui| {
                    ui.set_width(btn_w);
                    ui.set_max_width(btn_w);
                    if let Some(ratio) = paint_slider(ui, state, btn_w) {
                        *action = Some(UiAction::SetAttackRatio(ratio));
                    }
                });
            sow_ui_kit::theme::paint_hud_panel_gradient(
                ui,
                prepaint_idx,
                frame_res.response.rect,
                sow_ui_kit::theme::palette::field_border(),
                if compact {
                    egui::CornerRadius::ZERO
                } else {
                    sow_ui_kit::theme::radius::sm()
                },
            );
        });
}

/// Lay out %, vertical track + knob, and troop count in a single column of width `w`.
fn paint_slider(ui: &mut egui::Ui, state: &HudState, w: f32) -> Option<f32> {
    let mut changed = None;
    let dur = sow_ui_kit::theme::anim_duration_from_ctx(ui.ctx());

    let pct_h = 14.0;
    let troop_h = 13.0;
    let gap = 3.0;
    let widget_h = TRACK_H + KNOB_D;
    let total_h = pct_h + gap + widget_h + gap + troop_h;

    let (rect, _) = ui.allocate_exact_size(vec2(w, total_h), egui::Sense::hover());
    let (left, top) = (rect.left(), rect.top());

    let pct_rect = Rect::from_min_size(pos2(left, top), vec2(w, pct_h));
    let slider_outer = Rect::from_min_size(pos2(left, top + pct_h + gap), vec2(w, widget_h));
    let troop_rect = Rect::from_min_size(pos2(left, slider_outer.bottom() + gap), vec2(w, troop_h));

    let slider_id = ui.id().with("attack_ratio_slider");
    let track = Rect::from_center_size(slider_outer.center(), vec2(TRACK_W, TRACK_H));

    let response = ui.interact(slider_outer, slider_id, egui::Sense::click_and_drag());

    let mut ratio = state.attack_ratio;
    if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let new_ratio = ratio_from_track_y(track, pos.y);
            if (new_ratio - ratio).abs() > f32::EPSILON {
                ratio = new_ratio;
                changed = Some(ratio);
            }
        }
    } else if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            ratio = ratio_from_track_y(track, pos.y);
            changed = Some(ratio);
        }
    }

    let dragging = response.dragged() || response.is_pointer_button_down_on();
    let display_ratio = if dragging {
        ratio
    } else {
        ui.ctx()
            .animate_value_with_time(slider_id.with("ratio_anim"), state.attack_ratio, dur)
    };
    let display_troops = if dragging {
        (state.troops * ratio as f64).max(0.0) as f32
    } else {
        ui.ctx().animate_value_with_time(
            slider_id.with("troops_anim"),
            (state.troops * state.attack_ratio as f64).max(0.0) as f32,
            dur,
        )
    };

    let painter = ui.painter();
    if ui.is_rect_visible(pct_rect) {
        paint_centered_label(
            painter,
            pct_rect,
            &format!("{:.0}%", display_ratio * 100.0),
            11.0,
            sow_ui_kit::theme::palette::neon_cyan_hover(),
        );
    }
    if ui.is_rect_visible(troop_rect) {
        paint_centered_label(
            painter,
            troop_rect,
            &crate::utils::format_number(display_troops as f64),
            10.0,
            Color32::from_rgb(220, 230, 220),
        );
    }

    let is_hovered = response.hovered();
    let is_active = response.is_pointer_button_down_on();
    let hover_t = ui.ctx().animate_bool(slider_id.with("hover"), is_hovered);
    let active_t = ui.ctx().animate_bool(slider_id.with("active"), is_active);

    let rail_fill = sow_ui_kit::theme::palette::neon_cyan().linear_multiply(0.25);
    let rail_stroke = Stroke::new(
        1.0 + hover_t * 0.5 + active_t * 0.5,
        sow_ui_kit::theme::palette::neon_cyan().lerp_to_gamma(
            sow_ui_kit::theme::palette::neon_cyan_hover(),
            hover_t + active_t * 0.5,
        ),
    );
    painter.rect(
        track,
        CornerRadius::same((TRACK_W * 0.5) as u8),
        rail_fill,
        rail_stroke,
        egui::StrokeKind::Inside,
    );

    let knob_cy = knob_y(track, ratio);
    let fill_top = knob_cy.min(track.bottom());
    if fill_top < track.bottom() {
        let fill_rect = Rect::from_min_max(pos2(track.left(), fill_top), track.right_bottom());
        painter.rect(
            fill_rect,
            CornerRadius::same((TRACK_W * 0.5) as u8),
            sow_ui_kit::theme::palette::neon_cyan_hover().linear_multiply(0.55),
            Stroke::NONE,
            egui::StrokeKind::Inside,
        );
    }

    let knob_r = KNOB_D * 0.5;
    let knob_rect = Rect::from_center_size(pos2(track.center().x, knob_cy), vec2(KNOB_D, KNOB_D));
    let knob_fill = sow_ui_kit::theme::palette::field_bg().linear_multiply(0.92 + hover_t * 0.08);
    let knob_stroke = Stroke::new(
        1.5 + active_t,
        sow_ui_kit::theme::palette::neon_cyan_hover(),
    );
    painter.circle(knob_rect.center(), knob_r, knob_fill, knob_stroke);
    if hover_t > 0.01 || active_t > 0.01 {
        painter.circle(
            knob_rect.center(),
            knob_r + 2.0,
            Color32::TRANSPARENT,
            Stroke::new(
                2.0_f32,
                sow_ui_kit::theme::palette::neon_cyan()
                    .linear_multiply(hover_t * 0.35 + active_t * 0.25),
            ),
        );
    }

    let emoji_size = KNOB_D * 0.55;
    let emoji_rect = Rect::from_center_size(knob_rect.center(), vec2(emoji_size, emoji_size));
    if !crate::widgets::try_paint_emoji(painter, "⚔", emoji_rect, Color32::WHITE) {
        painter.text(
            knob_rect.center(),
            Align2::CENTER_CENTER,
            "⚔",
            egui::FontId::proportional(emoji_size),
            Color32::WHITE,
        );
    }

    changed
}
