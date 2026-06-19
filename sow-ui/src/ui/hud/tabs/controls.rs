use crate::UiAction;
use egui::{Color32, Slider, Stroke, vec2};
use sow_i18n::Language;

use super::battle_log;
use super::event_log;
use super::super::overlays::mobile;
use super::super::panels::troop_spawn;
use super::super::state::{BottomHudTab, HudState, building_emoji};

const ATTACK_RATIO_COL_W: f32 = 64.0;

pub(in crate::ui::hud) fn draw_buildings_strip(ui: &mut egui::Ui, state: &mut HudState, width: f32, compact: bool) {
    ui.set_width(width);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = if compact { 4.0 } else { 12.0 };

        let active_kinds = [
            sow_core::game::BuildingKind::City,
            sow_core::game::BuildingKind::Factory,
            sow_core::game::BuildingKind::Port,
            sow_core::game::BuildingKind::Bunker,
        ];
        let num_items = active_kinds.len() as f32;

        let mut available_width = width;
        if sow_core::config::ENABLE_MISSILE_STRUCTURES {
            let nuke_w = if compact { 32.0 } else { 36.0 };
            let extra_w = 4.0 + 8.0 + 4.0 + nuke_w + (if compact { 4.0 } else { 12.0 });
            available_width = (available_width - extra_w).max(50.0);
        }

        let col_w = (available_width - (num_items - 1.0) * (if compact { 4.0 } else { 12.0 })) / num_items;

        let btn_size = col_w.min(if compact { 40.0 } else { 48.0 });
        let gold_plate_h = if compact { 14.0 } else { 18.0 };
        let total_h = btn_size + 4.0 + gold_plate_h;

        for (display_idx, &kind) in active_kinds.iter().enumerate() {
            let cost_idx = sow_core::game::BuildingKind::ALL.iter().position(|&k| k == kind).unwrap_or(0);
            let cost = state.building_costs[cost_idx];
            let is_selected = state.selected_building_kind == Some(kind);
            let can_afford = state.gold >= cost;

            let tint = if is_selected {
                crate::ui::theme::palette::neon_cyan()
            } else if !can_afford {
                egui::Color32::from_rgb(180, 50, 50)
            } else {
                egui::Color32::WHITE
            };

            let (rect, mut resp) = ui.allocate_exact_size(
                egui::vec2(col_w, total_h),
                egui::Sense::click(),
            );

            resp = resp.on_hover_ui(|ui| {
                let name = match kind {
                    sow_core::game::BuildingKind::City => "City Center",
                    sow_core::game::BuildingKind::Bunker => "Defense Tower",
                    sow_core::game::BuildingKind::Factory => "Industrial Factory",
                    sow_core::game::BuildingKind::Port => "Maritime Port",
                };
                let desc = match kind {
                    sow_core::game::BuildingKind::City => "Core of your empire. Increases troop generation, gold generation, and max troops. Can be upgraded with 6 powerful modules (Port, Foundry, Armory, Intel, Arsenal, Shield)!",
                    sow_core::game::BuildingKind::Bunker => "Frontline Anchor: Fortifies borders, slowing enemy land grabs. Naturally strong on mountains (3x) and highlands (2x), upgradable with gold!",
                    sow_core::game::BuildingKind::Factory => "Economic Engine: A specialized pure gold generator. Upgradable up to Level 5 to progressively multiply gold income. Must be spaced from other structures.",
                    sow_core::game::BuildingKind::Port => "Maritime Port: Specialized coastal harbor. Generates gold and troop income and enables launching naval fleets. Must be built near the shore.",
                };

                ui.label(egui::RichText::new(name).strong().size(14.0).color(crate::ui::theme::palette::neon_cyan()));
                ui.add_space(4.0);
                ui.label(egui::RichText::new(desc).size(12.0).color(egui::Color32::LIGHT_GRAY));
                ui.add_space(6.0);

                let cost_text = if cost.is_infinite() { "N/A".to_string() } else { crate::utils::format_number(cost) };
                let cost_color = if can_afford { egui::Color32::from_rgb(74, 222, 128) } else { egui::Color32::from_rgb(239, 68, 68) };
                crate::widgets::emoji_label(
                    ui,
                    &format!("Cost: 🪙 {cost_text} Gold"),
                    egui::FontId::proportional(13.0),
                    cost_color,
                );
            });

            let square_rect = egui::Rect::from_min_size(
                egui::pos2(rect.center().x - btn_size * 0.5, rect.top()),
                egui::vec2(btn_size, btn_size),
            );

            let plate_rect = egui::Rect::from_center_size(
                egui::pos2(rect.center().x, square_rect.bottom() + 2.0 + gold_plate_h * 0.5),
                egui::vec2(btn_size.max(col_w * 0.9), gold_plate_h),
            );

            let is_hovered = resp.hovered();
            let card = crate::ui::theme::interact_card(
                is_selected,
                can_afford,
                is_hovered,
                crate::ui::theme::palette::neon_cyan(),
            );
            
            ui.painter().rect(
                square_rect,
                crate::ui::theme::radius::SM,
                egui::Color32::TRANSPARENT,
                card.stroke,
                egui::StrokeKind::Inside,
            );

            let icon_size = btn_size * 0.48;
            let icon_rect = egui::Rect::from_center_size(square_rect.center(), egui::vec2(icon_size, icon_size));
            if !crate::widgets::try_paint_emoji(ui.painter(), building_emoji(kind), icon_rect, tint) {
                ui.painter().text(
                    icon_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    building_emoji(kind),
                    egui::FontId::proportional(icon_size),
                    tint,
                );
            }

            let os = ui.ctx().os();
            let is_mobile_os = os == egui::os::OperatingSystem::IOS || os == egui::os::OperatingSystem::Android;
            if !is_mobile_os && !compact {
                let badge_size = 14.0;
                let badge_rect = egui::Rect::from_min_size(
                    egui::pos2(square_rect.left() + 2.0, square_rect.top() + 2.0),
                    egui::vec2(badge_size, badge_size),
                );
                let badge_bg = egui::Color32::from_rgba_unmultiplied(15, 20, 30, 240);
                let badge_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_white_alpha(80));
                ui.painter().rect(
                    badge_rect,
                    egui::CornerRadius::same((badge_size * 0.5) as u8),
                    badge_bg,
                    badge_stroke,
                    egui::StrokeKind::Inside,
                );
                ui.painter().text(
                    badge_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{}", display_idx + 1),
                    egui::FontId::proportional(9.0),
                    egui::Color32::WHITE,
                );
            }

            let plate_bg = egui::Color32::from_rgba_unmultiplied(10, 15, 25, 240);
            let plate_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_white_alpha(40));
            ui.painter().rect(
                plate_rect,
                egui::CornerRadius::same((gold_plate_h * 0.5) as u8),
                plate_bg,
                plate_stroke,
                egui::StrokeKind::Inside,
            );

            let cost_text = if cost.is_infinite() {
                "N/A".to_string()
            } else {
                crate::utils::format_number(cost)
            };

            let text_color = if !can_afford {
                egui::Color32::from_rgb(239, 68, 68)
            } else if is_selected {
                crate::ui::theme::palette::neon_cyan()
            } else {
                egui::Color32::from_rgb(230, 230, 230)
            };

            let cost_label = if cost_text == "N/A" {
                cost_text
            } else {
                format!("🪙 {cost_text}")
            };

            let font_size = if compact { 9.0 } else { 10.0 };
            crate::widgets::paint_emoji_text_at(
                ui.painter(),
                plate_rect.center(),
                egui::Align2::CENTER_CENTER,
                &cost_label,
                egui::FontId::proportional(font_size),
                text_color,
                false,
            );

            if resp.clicked() {
                if is_selected {
                    state.selected_building_kind = None;
                } else {
                    state.selected_building_kind = Some(kind);
                    state.selected_nuke_kind = None;
                }
            }
        }

        // Nuke launch buttons (separator + icons)
        if sow_core::config::ENABLE_MISSILE_STRUCTURES {
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            let nukes: [(sow_core::game::NukeKind, &str); 1] = [
                (sow_core::game::NukeKind::AtomBomb, "Nuke"),
            ];

            for &(nuke_kind, label) in &nukes {
                let is_selected = state.selected_nuke_kind == Some(nuke_kind);
                let nk_col_w = if compact { 32.0 } else { 36.0 };

                let tint = if is_selected {
                    egui::Color32::from_rgb(239, 68, 68)
                } else {
                    egui::Color32::WHITE
                };

                let bg_color = if is_selected {
                    egui::Color32::from_rgba_unmultiplied(239, 68, 68, 30)
                } else {
                    egui::Color32::from_rgba_unmultiplied(10, 15, 25, 120)
                };

                let stroke = if is_selected {
                    egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(239, 68, 68))
                } else {
                    egui::Stroke::new(1.0_f32, crate::ui::theme::palette::field_border().linear_multiply(0.5))
                };

                let (rect, mut resp) = ui.allocate_exact_size(
                    egui::vec2(nk_col_w, if compact { 38.0 } else { 44.0 }),
                    egui::Sense::click(),
                );

                resp = resp.on_hover_ui(|ui| {
                    let desc = "Missile payload that detonates on impact. Blast radius, flight speed, and size are upgraded by your city's Arsenal module level.";
                    ui.label(egui::RichText::new(label).strong().size(14.0).color(egui::Color32::from_rgb(239, 68, 68)));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(desc).size(12.0).color(egui::Color32::LIGHT_GRAY));
                });

                let final_bg = if resp.hovered() && !is_selected {
                    crate::ui::theme::palette::field_bg().linear_multiply(0.3)
                } else {
                    bg_color
                };

                ui.painter().rect(rect, 6, final_bg, stroke, egui::StrokeKind::Inside);

                // Hotkey badge (top-left corner)
                if !compact {
                    let hotkey_color = if is_selected {
                        egui::Color32::from_rgb(239, 68, 68)
                    } else {
                        egui::Color32::from_white_alpha(120)
                    };
                    ui.painter().text(
                        egui::pos2(rect.left() + 4.0, rect.top() + 4.0),
                        egui::Align2::LEFT_TOP,
                        "8".to_string(),
                        egui::FontId::proportional(7.0),
                        hotkey_color,
                    );
                }

                let icon_size = if compact { 16.0 } else { 20.0 };
                let icon_rect = egui::Rect::from_center_size(
                    egui::pos2(rect.center().x, rect.top() + (if compact { 11.0 } else { 14.0 })),
                    egui::vec2(icon_size, icon_size),
                );
                if !crate::widgets::try_paint_emoji(ui.painter(), "☢️", icon_rect, tint) {
                    ui.painter().text(
                        icon_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "☢️",
                        egui::FontId::proportional(icon_size * 0.7),
                        tint,
                    );
                }

                let font_size = if compact { 7.0 } else { 8.0 };
                ui.painter().text(
                    egui::pos2(rect.center().x, rect.bottom() - (if compact { 5.0 } else { 8.0 })),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(font_size),
                    if is_selected { egui::Color32::from_rgb(239, 68, 68) } else { egui::Color32::GRAY },
                );

                if resp.clicked() {
                    if is_selected {
                        state.selected_nuke_kind = None;
                    } else {
                        state.selected_nuke_kind = Some(nuke_kind);
                        state.selected_building_kind = None;
                    }
                }
            }
        }
    });
}

pub(in crate::ui::hud) fn tab_accent(tab: BottomHudTab) -> Color32 {
    match tab {
        BottomHudTab::Controls => crate::ui::theme::palette::neon_cyan(),
        BottomHudTab::BattleLog => crate::ui::theme::palette::danger(),
        BottomHudTab::EventLog => crate::ui::theme::palette::neon_gold_hover(),
    }
}

pub(in crate::ui::hud) fn draw_browser_tab_strip(
    ui: &mut egui::Ui,
    state: &mut HudState,
    compact: bool,
    dispatch_total: usize,
    event_unread: usize,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
) -> Option<egui::Rect> {
    let dispatch_unread = if state.bottom_tab != BottomHudTab::BattleLog {
        dispatch_total.saturating_sub(state.battle_log_seen_count)
    } else {
        0
    };

    let tab_count = 3.0_f32;
    let tab_w = ui.available_width() / tab_count;
    let mut active_rect = None;

    let strip_response = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = crate::ui::theme::tab::GAP;

        let tabs = [
            (BottomHudTab::Controls, 0_usize),
            (BottomHudTab::BattleLog, dispatch_unread),
            (BottomHudTab::EventLog, event_unread),
        ];

        for (tab, badge) in tabs {
            let selected = state.bottom_tab == tab;
            let resp = crate::ui::theme::draw_tab(ui, crate::ui::theme::TabContent::Icon(asset_loader.hud_icon(tab.hud_icon())), selected, tab_accent(tab), badge, tab_w, compact,);
            if selected {
                active_rect = Some(resp.rect);
            }
            if resp.clicked() {
                state.bottom_tab = tab;
                match tab {
                    BottomHudTab::BattleLog => {
                        state.battle_log_seen_count = dispatch_total;
                    }
                    BottomHudTab::EventLog => {
                        state.event_log_seen_count = state.event_log.len();
                    }
                    BottomHudTab::Controls => {}
                }
            }
        }
    });

    crate::ui::theme::draw_tab_baseline(ui, strip_response.response.rect, active_rect);
    active_rect
}
pub(in crate::ui::hud) fn hud_sidebar_row_height(compact: bool, spawn_active: bool, main: HudSidebarMain, main_w: f32) -> f32 {
    if spawn_active {
        return if compact { 72.0 } else { 56.0 };
    }
    let header_h = if compact { 24.0 } else { 22.0 };
    let row_gap = crate::ui::theme::margin::TIGHT as f32;
    let body_h = match main {
        HudSidebarMain::Controls => {
            let num_items = 4.0;
            let mut available_width = main_w;
            if sow_core::config::ENABLE_MISSILE_STRUCTURES {
                let nuke_w = if compact { 32.0 } else { 36.0 };
                let extra_w = 4.0 + 8.0 + 4.0 + nuke_w + (if compact { 4.0 } else { 12.0 });
                available_width = (available_width - extra_w).max(50.0);
            }
            let col_w = (available_width - (num_items - 1.0) * (if compact { 4.0 } else { 12.0 })) / num_items;
            let btn_size = col_w.min(if compact { 40.0 } else { 48.0 });
            let gold_plate_h = if compact { 14.0 } else { 18.0 };
            btn_size + 4.0 + gold_plate_h
        }
        HudSidebarMain::BattleLog | HudSidebarMain::EventLog => {
            if compact {
                120.0
            } else {
                140.0
            }
        }
    };
    body_h + row_gap + header_h
}

#[derive(Clone, Copy)]
pub(in crate::ui::hud) enum HudSidebarMain {
    Controls,
    BattleLog,
    EventLog,
}

pub(in crate::ui::hud) fn draw_attack_ratio_column(ui: &mut egui::Ui, state: &HudState, col_h: f32) -> Option<f32> {
    let ratio_troops = (state.troops * (state.attack_ratio as f64)).max(0.0);
    let mut changed_ratio = None;

    ui.allocate_ui_with_layout(
        vec2(ATTACK_RATIO_COL_W, col_h),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;

            // 1. Percentage label above slider
            crate::ui::theme::outlined_label(
                ui,
                &format!("{:.0}%", state.attack_ratio * 100.0),
                egui::FontId::proportional(11.0),
                crate::ui::theme::palette::neon_cyan_hover(),
            );

            // 2. Compact vertical slider in the middle
            let slider_h = (col_h - 32.0).clamp(16.0, 64.0);
            let mut ratio = state.attack_ratio;
            let changed = ui
                .scope(|ui| {
                    ui.spacing_mut().slider_width = slider_h;
                    ui.spacing_mut().slider_rail_height = 12.0; // wider/more noticeable rail
                    ui.visuals_mut().widgets.inactive.bg_fill = crate::ui::theme::palette::neon_cyan().linear_multiply(0.25);
                    ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(1.0_f32, crate::ui::theme::palette::neon_cyan());
                    ui.visuals_mut().widgets.hovered.bg_stroke = Stroke::new(1.5_f32, crate::ui::theme::palette::neon_cyan_hover());
                    ui.visuals_mut().widgets.active.bg_stroke = Stroke::new(1.5_f32, crate::ui::theme::palette::neon_cyan_hover());

                    let slider = Slider::new(&mut ratio, 0.01..=1.0)
                        .show_value(false)
                        .vertical();
                    ui.add_sized(vec2(24.0, slider_h), slider).changed()
                })
                .inner;
            if changed {
                changed_ratio = Some(ratio);
            }

            // 3. Troop quantity label below slider
            crate::ui::theme::outlined_label(
                ui,
                &crate::utils::format_number(ratio_troops),
                egui::FontId::proportional(10.0),
                Color32::from_rgb(220, 230, 220),
            );
        },
    );
    changed_ratio
}

#[allow(clippy::too_many_arguments)]
pub(in crate::ui::hud) fn draw_hud_sidebar_row(
    ui: &mut egui::Ui,
    state: &mut HudState,
    content_w: f32,
    compact: bool,
    action: &mut Option<UiAction>,
    lang: Language,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    main: HudSidebarMain,
) {
    let spawn_active = state.spawn_timer_secs.is_some();
    let show_ratio = !spawn_active;
    let ratio_gap = if show_ratio {
        crate::ui::theme::margin::COZY as f32
    } else {
        0.0
    };
    let main_w = content_w
        - if show_ratio {
            ATTACK_RATIO_COL_W + ratio_gap
        } else {
            0.0
        };
    let row_gap = crate::ui::theme::margin::TIGHT as f32;
    let row_h = hud_sidebar_row_height(compact, spawn_active, main, main_w);

    ui.allocate_ui_with_layout(
        vec2(content_w, row_h),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = ratio_gap;

            ui.allocate_ui_with_layout(
                vec2(main_w, row_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.spacing_mut().item_spacing.y = row_gap;
                    if !spawn_active {
                        match main {
                            HudSidebarMain::Controls => {
                                ui.push_id("controls_tab", |ui| {
                                    draw_controls_tab(
                                        ui,
                                        state,
                                        main_w,
                                        compact,
                                        cancel_intents,
                                        lang,
                                    );
                                });
                            }
                            HudSidebarMain::BattleLog => {
                                ui.push_id("battle_log_tab", |ui| {
                                    battle_log::draw_battle_log_tab(
                                        ui,
                                        state,
                                        main_w,
                                        compact,
                                        cancel_intents,
                                        lang,
                                    );
                                });
                            }
                            HudSidebarMain::EventLog => {
                                ui.push_id("event_log_tab", |ui| {
                                    event_log::draw_event_log_tab(ui, state, main_w, compact, lang);
                                });
                            }
                        }
                    }
                    ui.push_id("persistent_header", |ui| {
                        troop_spawn::draw_persistent_header(ui, state, compact, lang);
                    });
                },
            );

            if show_ratio {
                ui.separator();
                ui.push_id("attack_ratio_col", |ui| {
                    if let Some(ratio) = draw_attack_ratio_column(ui, state, row_h) {
                        *action = Some(UiAction::SetAttackRatio(ratio));
                    }
                });
            }
        },
    );
}

pub(in crate::ui::hud) fn draw_controls_with_attack_ratio(
    ui: &mut egui::Ui,
    state: &mut HudState,
    content_w: f32,
    compact: bool,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    lang: Language,
    action: &mut Option<UiAction>,
) {
    draw_hud_sidebar_row(
        ui,
        state,
        content_w,
        compact,
        action,
        lang,
        cancel_intents,
        HudSidebarMain::Controls,
    );
}

pub(in crate::ui::hud) fn draw_controls_tab(
    ui: &mut egui::Ui,
    state: &mut HudState,
    width: f32,
    compact: bool,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    lang: Language,
) {
    if state.spawn_timer_secs.is_none() {
        ui.push_id("building_strip", |ui| {
            draw_buildings_strip(ui, state, width, compact);
        });
    }

    if compact {
        if state.spawn_timer_secs.is_none() {
            ui.add_space(crate::ui::theme::margin::COZY as f32);
        }
        mobile::draw_mobile_selection_bar(ui, state, cancel_intents, lang);
    }
}
