use crate::UiAction;
use egui::{vec2, Align2, Color32};
use sow_i18n::Language;
use web_time::Instant;

use super::super::state::HudState;
use crate::ui::asset_loader::AssetLoader;

pub(in crate::ui::hud) fn draw_top_icons(
    ui: &mut egui::Ui,
    state: &mut HudState,
    lang: Language,
    action: &mut Option<UiAction>,
    _asset_loader: &mut AssetLoader,
) {
    // ── Top-right HUD buttons ─────────────────────────────────────────────────
    let my_snapshot = state.players.iter().find(|p| p.id == state.my_player_id);
    let requests = my_snapshot
        .map(|p| p.alliance_requests.clone())
        .unwrap_or_default();
    let resource_requests = my_snapshot
        .map(|p| p.resource_requests.clone())
        .unwrap_or_default();

    let total_notifications = requests.len() + resource_requests.len();

    // Auto-open if a new request pops (only if it is the first/only request)
    let mut has_new_request = false;
    for &req_id in &requests {
        if !state.prev_requests.contains(&req_id) {
            has_new_request = true;
            break;
        }
    }
    for req in &resource_requests {
        if !state.prev_resource_requests.contains(&req.requester) {
            has_new_request = true;
            break;
        }
    }
    if has_new_request {
        state.last_request_time = Some(Instant::now());
        if total_notifications <= 1 {
            state.show_alliance_inbox = true;
        }
    }
    state.prev_requests = requests.clone();
    state.prev_resource_requests = resource_requests.iter().map(|r| r.requester).collect();

    egui::Area::new(egui::Id::new("hud_top_icons"))
        .anchor(Align2::RIGHT_TOP, vec2(-12.0, 12.0 + state.safe_area_top))
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
            let prepaint_idx = ui.painter().add(egui::Shape::Noop);
            let frame_res = egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(
                    sow_ui_kit::theme::margin::COZY,
                    sow_ui_kit::theme::margin::TIGHT,
                ))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let btn_resp = ui
                            .add(crate::widgets::HudEmojiButton::new("📩"))
                            .on_hover_text(&sow_i18n::get(lang).hud.inbox_title);
                        if btn_resp.clicked() {
                            state.show_alliance_inbox = !state.show_alliance_inbox;
                        }

                        if total_notifications > 0 {
                            let mut scale = 1.0_f32;
                            if let Some(t) = state.last_request_time {
                                let elapsed = t.elapsed().as_secs_f32();
                                if elapsed < 0.6_f32 {
                                    let progress = elapsed / 0.6_f32;
                                    scale = 1.0_f32
                                        + 0.8_f32
                                            * (progress * std::f32::consts::PI).sin()
                                            * (1.0_f32 - progress);
                                    ui.ctx().request_repaint();
                                }
                            }

                            let badge_center = btn_resp.rect.right_top() + egui::vec2(-2.0, 2.0);
                            sow_ui_kit::theme::paint_count_badge(
                                ui.painter(),
                                badge_center,
                                total_notifications,
                                8.0_f32 * scale,
                                10.0_f32 * scale,
                                None,
                            );
                        }

                        if ui
                            .add(crate::widgets::HudEmojiButton::new("⚙"))
                            .on_hover_text(&sow_i18n::get(lang).hud.hover_settings)
                            .clicked()
                        {
                            *action = Some(UiAction::ToggleSettings);
                        }
                        if ui
                            .add(
                                crate::widgets::HudEmojiButton::new("❌")
                                    .color(Color32::from_rgb(255, 100, 100)),
                            )
                            .on_hover_text(&sow_i18n::get(lang).hud.hover_exit)
                            .clicked()
                        {
                            if state.is_tutorial {
                                *action = Some(UiAction::LeaveLobby);
                            } else {
                                state.show_exit_confirm = true;
                            }
                        }

                        let top_icons_rect = ui.min_rect();
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(egui::Id::new("hud_top_icons_rect"), top_icons_rect);
                        });
                    });
                });
            let compact = sow_ui_kit::theme::compact_viewport(ui.ctx());
            sow_ui_kit::theme::paint_hud_panel_gradient(
                ui,
                prepaint_idx,
                frame_res.response.rect,
                sow_ui_kit::theme::palette::field_border(),
                if compact {
                    egui::CornerRadius::ZERO
                } else {
                    sow_ui_kit::theme::radius::md()
                },
            );
        });
}
