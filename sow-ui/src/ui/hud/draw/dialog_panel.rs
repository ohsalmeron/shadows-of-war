//! The "In and Out" dialog panel: a message **pinned just above the bottom HUD panel**, animating
//! in and out. It is a visual *clone* of the bottom panel (same surface fill, hairline border,
//! corner radius and margins) and renders its text + emoji through the GPU atlas pipeline via
//! [`crate::widgets::paint_dialog_contents`] — the same path the HUD uses, now demonstrated on a
//! modal-style surface.
//!
//! Backend lives in [`HudState`]: the caller queues [`HudState::bottom_dialog`] and reads back the
//! clicked button (tagged with the dialog id) from [`HudState::bottom_dialog_click`]. This panel
//! owns presentation only — the in/out animation, tap-to-skip and timed auto-dismiss.

use super::super::state::HudState;
use crate::ui::asset_loader::AssetLoader;

pub(in crate::ui::hud) fn draw_pinned_dialog(
    ui: &mut egui::Ui,
    state: &mut HudState,
    asset_loader: &AssetLoader,
) {
    let ctx = ui.ctx().clone();

    let active = state.bottom_dialog.is_some();
    if active {
        state.bottom_dialog_display = state.bottom_dialog.clone();
    }

    let dur = sow_ui_kit::theme::anim_duration_from_ctx(&ctx);
    let t = ctx.animate_bool_with_time(egui::Id::new("hud_pinned_dialog_t"), active, dur);
    if t > 0.0 && t < 1.0 {
        ctx.request_repaint();
    }
    if t <= 0.0 && !active {
        state.bottom_dialog_display = None;
        state.bottom_dialog_click = None;
        return;
    }
    let Some(dlg) = state.bottom_dialog_display.clone() else {
        state.bottom_dialog_click = None;
        return;
    };

    let screen = ctx.content_rect();
    let compact = sow_ui_kit::theme::compact_viewport(&ctx);
    let portrait_dock = sow_ui_kit::theme::portrait_layout(&ctx);

    // Clone the bottom panel's width + chrome so the dialog reads as its twin.
    let panel_w = if portrait_dock {
        screen.width()
    } else {
        520.0_f32.min(screen.width() - 24.0)
    };
    let content_margin = if portrait_dock || compact {
        egui::Margin {
            left: sow_ui_kit::theme::margin::COZY,
            right: sow_ui_kit::theme::margin::COZY,
            top: sow_ui_kit::theme::margin::COZY,
            bottom: sow_ui_kit::theme::margin::COZY,
        }
    } else {
        egui::Margin::same(sow_ui_kit::theme::margin::REGULAR)
    };

    // Spring slide-up + fade — the same idiom the transfer/emoji panels use.
    let scale = if active {
        if t >= 1.0 {
            1.0
        } else {
            crate::ui::animation::spring_overshoot(t)
        }
    } else {
        t
    };
    let slide = 16.0 * (1.0 - scale.clamp(0.0, 1.0));
    let alpha = t.clamp(0.0, 1.0);

    // Sit *on* the bottom panel: same anchor + offset, so the dialog covers it (its taller content
    // extends upward). Mirrors the bottom panel's anchoring in `hud::draw`.
    let anchor = if portrait_dock {
        egui::Align2::LEFT_BOTTOM
    } else {
        egui::Align2::CENTER_BOTTOM
    };
    let lift = if portrait_dock {
        state.safe_area_bottom
    } else {
        state.safe_area_bottom + 12.0
    };
    let offset = egui::vec2(0.0, -lift + slide);

    let settled = active && t >= 0.999;

    let area = egui::Area::new(egui::Id::new("hud_pinned_dialog"))
        .order(egui::Order::Foreground)
        .anchor(anchor, offset)
        .movable(false)
        .show(&ctx, |ui| {
            ui.set_opacity(alpha);
            ui.set_max_width(panel_w);
            let frame = egui::Frame::NONE
                .fill(sow_ui_kit::theme::palette::surface())
                .stroke(egui::Stroke::new(
                    sow_ui_kit::theme::stroke::HAIRLINE,
                    sow_ui_kit::theme::palette::field_border(),
                ))
                .corner_radius(sow_ui_kit::theme::radius::lg())
                .inner_margin(content_margin);
            let fr = frame.show(ui, |ui| {
                ui.set_width(panel_w);
                crate::widgets::paint_dialog_contents(
                    ui,
                    dlg.visual.as_ref(),
                    dlg.name.as_deref(),
                    &dlg.title,
                    &dlg.body,
                    &dlg.buttons,
                    asset_loader,
                    compact,
                )
            });
            // Buttons + tap-to-skip both live on this Foreground layer. The full-panel interact
            // also *blocks* clicks from falling through to the bottom panel's controls, which now
            // sit hidden directly behind this one.
            let mut click = if settled { fr.inner } else { None };
            let blocker = ui.interact(
                fr.response.rect,
                egui::Id::new("hud_pinned_dialog_block"),
                egui::Sense::click(),
            );
            if settled && dlg.click_anywhere {
                if click.is_none() && blocker.clicked() {
                    click = Some(0);
                }
                if blocker.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
            click
        });

    let mut click = area.inner;

    // Timed auto-dismiss ("limited time"): fire button 0 once the hold elapses.
    if let Some(secs) = dlg.auto_dismiss_secs {
        let started_id = egui::Id::new("hud_pinned_dialog_started_at");
        let started_for = egui::Id::new("hud_pinned_dialog_started_for");
        let now = ctx.input(|i| i.time);
        let same =
            ctx.data(|d| d.get_temp::<String>(started_for)).as_deref() == Some(dlg.id.as_str());
        let started = if same {
            ctx.data(|d| d.get_temp::<f64>(started_id)).unwrap_or(now)
        } else {
            ctx.data_mut(|d| {
                d.insert_temp(started_for, dlg.id.clone());
                d.insert_temp(started_id, now);
            });
            now
        };
        if settled {
            if (now - started) as f32 >= secs {
                click = click.or(Some(0));
            } else {
                ctx.request_repaint(); // keep the hold timer advancing
            }
        }
    }

    if let Some(idx) = click {
        state.bottom_dialog_click = Some((dlg.id.clone(), idx));
    }
}
