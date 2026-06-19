use crate::UiAction;
use egui::Align2;
use sow_i18n::Language;

use super::super::state::{BottomHudTab, HudState, hud_map_controls_anchor_offset};

pub(in crate::ui::hud) fn draw_map_controls(
    ui: &mut egui::Ui,
    state: &mut HudState,
    lang: Language,
    compact: bool,
    log_tabs_enabled: bool,
    action: &mut Option<UiAction>,
) {
    // ── Floating Map Controls ──────────────────────────────────────────────
    let map_controls_offset =
        hud_map_controls_anchor_offset(ui.ctx(), compact, state.safe_area_bottom);
    egui::Area::new(egui::Id::new("hud_map_controls"))
        .anchor(Align2::RIGHT_BOTTOM, map_controls_offset)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            let btn_w = if cfg!(target_os = "android") {
                46.0
            } else {
                30.0
            };
            // Area gives full screen width unless capped — otherwise the frame paints a fat bar.
            let rail_pad_x = 4.0;
            let rail_w = btn_w + rail_pad_x * 2.0;
            ui.set_width(rail_w);
            ui.set_max_width(rail_w);

            crate::ui::theme::panel_frame(crate::ui::theme::PanelKind::MapControlsRail, compact)
                .show(ui, |ui| {
                    ui.set_width(btn_w);
                    ui.set_max_width(btn_w);
                    ui.spacing_mut().item_spacing.y = crate::ui::theme::margin::TIGHT as f32;
                    ui.vertical(|ui| {
                        if ui
                            .add(crate::widgets::HudEmojiButton::new("➕").dim(btn_w))
                            .on_hover_text(&sow_i18n::get(lang).hud.hover_zoom_in)
                            .clicked()
                        {
                            *action = Some(UiAction::ZoomIn);
                        }
                        if ui
                            .add(crate::widgets::HudEmojiButton::new("➖").dim(btn_w))
                            .on_hover_text(&sow_i18n::get(lang).hud.hover_zoom_out)
                            .clicked()
                        {
                            *action = Some(UiAction::ZoomOut);
                        }
                        if ui
                            .add(crate::widgets::HudEmojiButton::new("🏠").dim(btn_w))
                            .on_hover_text(&sow_i18n::get(lang).hud.hover_center_camera)
                            .clicked()
                        {
                            *action = Some(UiAction::CenterCamera);
                        }
                        ui.separator();
                        if !state.chat_disabled
                            && ui
                                .add(crate::widgets::HudEmojiButton::new("😀").dim(btn_w))
                                .on_hover_text(&sow_i18n::get(lang).hud.hover_express_emoji)
                                .clicked()
                        {
                            state.show_emoji_panel = !state.show_emoji_panel;
                            if state.show_emoji_panel {
                                state.emoji_panel_pos = None;
                                state.emoji_panel_just_opened = true;
                            }
                        }
                        if log_tabs_enabled {
                            let my_pid = state.my_player_id;
                            let total_attacks = if my_pid != 0 {
                                state
                                    .attacks
                                    .iter()
                                    .filter(|a| a.target_owner == my_pid || a.owner_id == my_pid)
                                    .count()
                                    + state.fleets.iter().filter(|f| f.owner_id == my_pid).count()
                            } else {
                                0
                            };

                            let attacks_btn = ui
                                .add(crate::widgets::HudEmojiButton::new("⚔").dim(btn_w))
                                .on_hover_text(&sow_i18n::get(lang).hud.hover_battle_log);
                            if attacks_btn.clicked() {
                                state.bottom_tab = BottomHudTab::BattleLog;
                                state.battle_log_seen_count = total_attacks;
                            }

                            let battle_unread = if state.bottom_tab != BottomHudTab::BattleLog {
                                total_attacks.saturating_sub(state.battle_log_seen_count)
                            } else {
                                0
                            };

                            if battle_unread > 0 {
                                let badge_center =
                                    attacks_btn.rect.right_top() + egui::vec2(-2.0, 2.0);
                                crate::ui::theme::paint_count_badge(
                                    ui.painter(),
                                    badge_center,
                                    battle_unread,
                                    6.5,
                                    8.5,
                                    Some(9),
                                );
                            }
                        }
                    });
                });
        });
}

