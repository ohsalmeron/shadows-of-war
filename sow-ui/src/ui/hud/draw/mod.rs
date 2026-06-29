use crate::UiAction;
use sow_i18n::Language;

use super::overlays::{betrayal, exit, sync};
use super::panels::transfer;
use super::state::{dispatch_count, incoming_dispatch_count, BottomHudTab, HudState};
use crate::ui::asset_loader::AssetLoader;

mod bottom;
mod emoji;
mod inbox;
mod map_controls;
mod top_icons;
mod attack_ratio;

use bottom::draw_bottom_panel;
use emoji::draw_emoji_panel;
use inbox::draw_alliance_inbox;
use map_controls::draw_map_controls;
use top_icons::draw_top_icons;
use attack_ratio::draw_attack_ratio_rail;

pub fn draw(
    ui: &mut egui::Ui,
    state: &mut HudState,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    lang: Language,
    asset_loader: &mut AssetLoader,
) -> Option<UiAction> {
    static REGISTER_ONCE: std::sync::Once = std::sync::Once::new();
    REGISTER_ONCE.call_once(|| {
        sow_ui_kit::register_game_assets(ui.ctx());
    });

    let mut action = None;

    let rect = ui.ctx().content_rect();
    let compact = sow_ui_kit::theme::compact_viewport(ui.ctx());
    let portrait_dock = sow_ui_kit::theme::portrait_layout(ui.ctx());
    let anim = sow_ui_kit::theme::anim_duration_from_ctx(ui.ctx());
    let anim_hover = sow_ui_kit::theme::anim_duration_hover_from_ctx(ui.ctx());

    let panel_w = if portrait_dock {
        rect.width()
    } else {
        520.0_f32.min(rect.width() - 24.0)
    };

    let log_tabs_enabled = sow_core::config::ENABLE_BOTTOM_HUD_LOG_TABS;
    let dispatch_total = dispatch_count(state);
    let incoming = incoming_dispatch_count(state);
    if log_tabs_enabled {
        if incoming > state.prev_incoming_dispatch_count
            && state.bottom_tab != BottomHudTab::BattleLog
        {
            state.bottom_tab = BottomHudTab::BattleLog;
        }
        state.prev_incoming_dispatch_count = incoming;

        match state.bottom_tab {
            BottomHudTab::BattleLog => state.battle_log_seen_count = dispatch_total,
            BottomHudTab::EventLog => state.event_log_seen_count = state.event_log.len(),
            BottomHudTab::Controls => {}
        }
    } else {
        state.bottom_tab = BottomHudTab::Controls;
        state.prev_incoming_dispatch_count = incoming;
    }

    let event_unread = if log_tabs_enabled && state.bottom_tab != BottomHudTab::EventLog {
        state
            .event_log
            .len()
            .saturating_sub(state.event_log_seen_count)
    } else {
        0
    };

    let (bottom_anchor, bottom_offset) = if portrait_dock {
        // Mobile dock: flush to the bottom edge (above the OS safe area) by design.
        (
            egui::Align2::LEFT_BOTTOM,
            egui::vec2(0.0, -state.safe_area_bottom),
        )
    } else {
        // Desktop/landscape: lift the floating panel off the viewport bottom a touch.
        const BOTTOM_LIFT: f32 = 12.0;
        (
            egui::Align2::CENTER_BOTTOM,
            egui::vec2(0.0, -state.safe_area_bottom - BOTTOM_LIFT),
        )
    };
    let panel_radius = if portrait_dock {
        sow_ui_kit::theme::radius::dock_top()
    } else {
        sow_ui_kit::theme::radius::lg()
    };

    draw_bottom_panel(
        ui,
        state,
        cancel_intents,
        lang,
        asset_loader,
        &mut action,
        portrait_dock,
        compact && portrait_dock,
        panel_w,
        log_tabs_enabled,
        dispatch_total,
        event_unread,
        bottom_anchor,
        bottom_offset,
        panel_radius,
    );
    draw_top_icons(ui, state, lang, &mut action, asset_loader);
    draw_alliance_inbox(ui, state, cancel_intents, lang, anim, asset_loader);
    draw_map_controls(ui, state, lang, compact, log_tabs_enabled, &mut action);
    draw_attack_ratio_rail(ui, state, compact, &mut action);
    draw_emoji_panel(ui, state, cancel_intents, lang, compact, anim, anim_hover);
    transfer::draw_transfer_panel(ui, state, cancel_intents, lang);
    sync::draw_sync_overlay(ui.ctx(), state, lang);
    betrayal::draw_betrayal_overlay(ui.ctx(), state, cancel_intents, lang, asset_loader);


    if let Some(act) = exit::draw_exit_confirm_overlay(ui.ctx(), state, lang) {
        action = Some(act);
    }

    action
}
