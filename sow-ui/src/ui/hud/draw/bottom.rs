use crate::UiAction;
use egui::{vec2, Align2};
use sow_i18n::Language;

use super::super::state::{BottomHudTab, HudState};
use super::super::tabs::controls;
use crate::ui::asset_loader::AssetLoader;

#[allow(clippy::too_many_arguments)]
pub(in crate::ui::hud) fn draw_bottom_panel(
    ui: &mut egui::Ui,
    state: &mut HudState,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    lang: Language,
    asset_loader: &mut AssetLoader,
    action: &mut Option<UiAction>,
    portrait_dock: bool,
    compact: bool,
    panel_w: f32,
    log_tabs_enabled: bool,
    dispatch_total: usize,
    event_unread: usize,
    bottom_anchor: Align2,
    bottom_offset: egui::Vec2,
    panel_radius: egui::CornerRadius,
) {
    let bottom_hud_area = egui::Area::new(egui::Id::new("hud_bottom_area_v9"))
        .anchor(bottom_anchor, bottom_offset)
        .order(egui::Order::Foreground)
        .movable(false)
        .show(ui.ctx(), |ui| {
            ui.set_max_width(panel_w);

            let border_color =
                if state.selected_building_kind.is_some() || state.selected_nuke_kind.is_some() {
                    sow_ui_kit::theme::palette::neon_cyan()
                } else {
                    sow_ui_kit::theme::palette::field_border()
                };

            let content_margin = if portrait_dock || compact {
                egui::Margin {
                    left: sow_ui_kit::theme::margin::COZY,
                    right: sow_ui_kit::theme::margin::COZY,
                    top: sow_ui_kit::theme::margin::COZY,
                    bottom: sow_ui_kit::theme::margin::TIGHT,
                }
            } else {
                egui::Margin {
                    left: sow_ui_kit::theme::margin::REGULAR,
                    right: sow_ui_kit::theme::margin::REGULAR,
                    top: sow_ui_kit::theme::margin::REGULAR,
                    bottom: sow_ui_kit::theme::margin::TIGHT,
                }
            };

            let prepaint_idx = ui.painter().add(egui::Shape::Noop);

            let frame_res = egui::Frame::NONE
                .inner_margin(content_margin)
                .show(ui, |ui| {
                    ui.allocate_ui_with_layout(
                        vec2(panel_w, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_width(panel_w);
                            ui.spacing_mut().item_spacing.y =
                                sow_ui_kit::theme::margin::COZY as f32;

                            if log_tabs_enabled {
                                ui.push_id("tab_strip", |ui| {
                                    controls::draw_browser_tab_strip(
                                        ui,
                                        state,
                                        compact,
                                        dispatch_total,
                                        event_unread,
                                        asset_loader,
                                    );
                                });
                            }

                            let content_w = ui.available_width();

                             if log_tabs_enabled {
                                 match state.bottom_tab {
                                     BottomHudTab::Controls => {
                                         controls::draw_controls_with_attack_ratio(
                                             ui,
                                             state,
                                             content_w,
                                             compact,
                                             cancel_intents,
                                             lang,
                                             action,
                                             asset_loader,
                                         );
                                     }
                                     BottomHudTab::BattleLog => {
                                         controls::draw_hud_sidebar_row(
                                             ui,
                                             state,
                                             content_w,
                                             compact,
                                             action,
                                             lang,
                                             cancel_intents,
                                             controls::HudSidebarMain::BattleLog,
                                             asset_loader,
                                         );
                                     }
                                     BottomHudTab::EventLog => {
                                         controls::draw_hud_sidebar_row(
                                             ui,
                                             state,
                                             content_w,
                                             compact,
                                             action,
                                             lang,
                                             cancel_intents,
                                             controls::HudSidebarMain::EventLog,
                                             asset_loader,
                                         );
                                     }
                                 }
                             } else {
                                 controls::draw_controls_with_attack_ratio(
                                     ui,
                                     state,
                                     content_w,
                                     compact,
                                     cancel_intents,
                                     lang,
                                     action,
                                     asset_loader,
                                 );
                             }
                        },
                    );
                });

            sow_ui_kit::theme::paint_hud_panel_gradient(
                ui,
                prepaint_idx,
                frame_res.response.rect,
                border_color,
                panel_radius,
            );
        });
    ui.ctx().data_mut(|d| {
        d.insert_temp(
            egui::Id::new("hud_bottom_panel_rect"),
            bottom_hud_area.response.rect,
        );
    });
}
