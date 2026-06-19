use crate::UiAction;
use egui::{Align2, vec2};
use sow_i18n::Language;

use super::super::state::{BottomHudTab, HudState};
use super::super::tabs::controls;
use crate::ui::asset_loader::AssetLoader;

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
                    crate::ui::theme::palette::neon_cyan()
                } else {
                    crate::ui::theme::palette::field_border().linear_multiply(0.4)
                };

            let content_margin = if portrait_dock || compact {
                egui::Margin {
                    left: crate::ui::theme::margin::COZY,
                    right: crate::ui::theme::margin::COZY,
                    top: crate::ui::theme::margin::COZY,
                    bottom: crate::ui::theme::margin::TIGHT,
                }
            } else {
                egui::Margin {
                    left: crate::ui::theme::margin::REGULAR,
                    right: crate::ui::theme::margin::REGULAR,
                    top: crate::ui::theme::margin::REGULAR,
                    bottom: crate::ui::theme::margin::TIGHT,
                }
            };

            egui::Frame::NONE
                .fill(crate::ui::theme::palette::field_bg())
                .stroke(egui::Stroke::new(
                    crate::ui::theme::stroke::HAIRLINE,
                    border_color,
                ))
                .corner_radius(panel_radius)
                .inner_margin(content_margin)
                .show(ui, |ui| {
                    ui.allocate_ui_with_layout(
                        vec2(panel_w, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_width(panel_w);
                            ui.spacing_mut().item_spacing.y = crate::ui::theme::margin::COZY as f32;

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
                                );
                            }
                        },
                    );
                });
        });
    ui.ctx().data_mut(|d| {
        d.insert_temp(
            egui::Id::new("hud_bottom_panel_rect"),
            bottom_hud_area.response.rect,
        );
    });

}
