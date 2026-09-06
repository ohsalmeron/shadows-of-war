use egui::{Align2, vec2};
use sow_i18n::Language;

use super::super::state::{BottomHudTab, HudState};
use super::super::tabs::controls;
use crate::ui::asset_loader::AssetLoader;

pub(in crate::ui::hud) struct BottomPanelOpts<'a> {
    pub cancel_intents: &'a mut Vec<sow_core::protocol::GameplayIntent>,
    pub lang: Language,
    pub asset_loader: &'a mut AssetLoader,
    pub portrait_dock: bool,
    pub compact: bool,
    pub panel_w: f32,
    pub log_tabs_enabled: bool,
    pub dispatch_total: usize,
    pub event_unread: usize,
    pub bottom_anchor: Align2,
    pub bottom_offset: egui::Vec2,
    pub panel_radius: egui::CornerRadius,
}

pub(in crate::ui::hud) fn draw_bottom_panel(
    ui: &mut egui::Ui,
    state: &mut HudState,
    opts: BottomPanelOpts<'_>,
) {
    let cancel_intents = opts.cancel_intents;
    let lang = opts.lang;
    let asset_loader = opts.asset_loader;
    let portrait_dock = opts.portrait_dock;
    let compact = opts.compact;
    let panel_w = opts.panel_w;
    let log_tabs_enabled = opts.log_tabs_enabled;
    let dispatch_total = opts.dispatch_total;
    let event_unread = opts.event_unread;
    let bottom_anchor = opts.bottom_anchor;
    let bottom_offset = opts.bottom_offset;
    let panel_radius = opts.panel_radius;
    let bottom_hud_area = egui::Area::new(egui::Id::new("hud_bottom_area_v9"))
        .anchor(bottom_anchor, bottom_offset)
        .order(egui::Order::Foreground)
        .movable(false)
        .show(ui.ctx(), |ui| {
            ui.set_max_width(panel_w);
            ui.style_mut().override_text_style = Some(egui::TextStyle::Small);

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
                        vec2(ui.available_width(), 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_width(ui.available_width());
                            ui.spacing_mut().item_spacing.y =
                                sow_ui_kit::theme::margin::COZY as f32;

                            if log_tabs_enabled && !(compact && state.bottom_dialog.is_some()) {
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
                                        controls::draw_controls_row(
                                            ui,
                                            state,
                                            controls::HudSidebarOpts {
                                                content_w,
                                                compact,
                                                lang,
                                                cancel_intents,
                                                main: controls::HudSidebarMain::Controls,
                                                asset_loader,
                                            },
                                        );
                                    }
                                    BottomHudTab::BattleLog => {
                                        controls::draw_hud_sidebar_row(
                                            ui,
                                            state,
                                            controls::HudSidebarOpts {
                                                content_w,
                                                compact,
                                                lang,
                                                cancel_intents,
                                                main: controls::HudSidebarMain::BattleLog,
                                                asset_loader,
                                            },
                                        );
                                    }
                                    BottomHudTab::EventLog => {
                                        controls::draw_hud_sidebar_row(
                                            ui,
                                            state,
                                            controls::HudSidebarOpts {
                                                content_w,
                                                compact,
                                                lang,
                                                cancel_intents,
                                                main: controls::HudSidebarMain::EventLog,
                                                asset_loader,
                                            },
                                        );
                                    }
                                }
                            } else {
                                controls::draw_controls_row(
                                    ui,
                                    state,
                                    controls::HudSidebarOpts {
                                        content_w,
                                        compact,
                                        lang,
                                        cancel_intents,
                                        main: controls::HudSidebarMain::Controls,
                                        asset_loader,
                                    },
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
