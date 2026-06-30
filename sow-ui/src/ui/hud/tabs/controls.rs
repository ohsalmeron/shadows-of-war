use crate::UiAction;
use egui::{vec2, Color32};
use sow_i18n::Language;

use super::super::overlays::mobile;
use super::super::panels::troop_spawn;
use super::super::state::{building_emoji, BottomHudTab, HudState};

enum StripItem {
    Building(sow_core::game::BuildingKind),
    Nuke(sow_core::game::NukeKind),
}
use super::battle_log;
use super::event_log;

pub(in crate::ui::hud) fn draw_buildings_strip(
    ui: &mut egui::Ui,
    state: &mut HudState,
    width: f32,
    compact: bool,
) {
    ui.set_width(width);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = if compact { 4.0 } else { 12.0 };
        let spacing = ui.spacing().item_spacing.x;

        let mut items: Vec<StripItem> = vec![
            StripItem::Building(sow_core::game::BuildingKind::City),
            StripItem::Building(sow_core::game::BuildingKind::Factory),
            StripItem::Building(sow_core::game::BuildingKind::Port),
            StripItem::Building(sow_core::game::BuildingKind::Bunker),
        ];
        if sow_core::config::ENABLE_MISSILE_STRUCTURES {
            items.push(StripItem::Nuke(sow_core::game::NukeKind::AtomBomb));
        }
        let num_items = items.len() as f32;

        let col_w = (width - (num_items - 1.0) * spacing) / num_items;
        let btn_size = col_w.min(if compact { 40.0 } else { 48.0 });
        let gold_plate_h = if compact { 14.0 } else { 18.0 };
        let total_h = btn_size + 4.0 + gold_plate_h;

        for (display_idx, item) in items.iter().enumerate() {
            let (is_nuke, is_selected, can_afford, tint, emoji, cost_label, accent, hotkey) = match item {
                StripItem::Building(kind) => {
                    let cost_idx = sow_core::game::BuildingKind::ALL.iter().position(|&k| k == *kind).unwrap_or(0);
                    let cost = state.building_costs[cost_idx];
                    let sel = state.selected_building_kind == Some(*kind);
                    let afford = state.gold >= cost;
                    let t = if sel {
                        sow_ui_kit::theme::palette::neon_cyan()
                    } else if !afford {
                        egui::Color32::from_rgb(180, 50, 50)
                    } else {
                        egui::Color32::WHITE
                    };
                    let cost_txt = if cost.is_infinite() { "N/A".to_string() } else { crate::utils::format_number(cost) };
                    let cl = if cost_txt == "N/A" { cost_txt } else { format!("🪙 {cost_txt}") };
                    (false, sel, afford, t, building_emoji(*kind).to_owned(), cl, sow_ui_kit::theme::palette::neon_cyan(), format!("{}", display_idx + 1))
                }
                StripItem::Nuke(kind) => {
                    let sel = state.selected_nuke_kind == Some(*kind);
                    let red = egui::Color32::from_rgb(239, 68, 68);
                    (true, sel, true, if sel { red } else { egui::Color32::WHITE }, "☢️".to_owned(), "Nuke".to_owned(), red, "8".to_owned())
                }
            };

            let (rect, mut resp) = ui.allocate_exact_size(
                egui::vec2(col_w, total_h),
                egui::Sense::click(),
            );

            resp = resp.on_hover_ui(|ui| {
                if is_nuke {
                    crate::widgets::outlined_emoji_label(ui, "Nuke", egui::FontId::proportional(14.0), accent);
                    ui.add_space(4.0);
                    crate::widgets::emoji_label(ui, "Missile payload that detonates on impact. Blast radius, flight speed, and size are upgraded by your city's Arsenal module level.", egui::FontId::proportional(12.0), egui::Color32::LIGHT_GRAY);
                } else {
                    let kind = match item { StripItem::Building(k) => *k, _ => unreachable!() };
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
                    let cost_idx = sow_core::game::BuildingKind::ALL.iter().position(|&k| k == kind).unwrap_or(0);
                    let cost = state.building_costs[cost_idx];
                    crate::widgets::outlined_emoji_label(ui, name, egui::FontId::proportional(14.0), sow_ui_kit::theme::palette::neon_cyan());
                    ui.add_space(4.0);
                    crate::widgets::emoji_label(ui, desc, egui::FontId::proportional(12.0), egui::Color32::LIGHT_GRAY);
                    ui.add_space(6.0);
                    let cost_text = if cost.is_infinite() { "N/A".to_string() } else { crate::utils::format_number(cost) };
                    let cost_color = if can_afford { egui::Color32::from_rgb(74, 222, 128) } else { egui::Color32::from_rgb(239, 68, 68) };
                    crate::widgets::emoji_label(ui, &format!("Cost: 🪙 {cost_text} Gold"), egui::FontId::proportional(13.0), cost_color);
                }
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
            let card = sow_ui_kit::theme::interact_card(
                is_selected,
                can_afford,
                is_hovered,
                accent,
            );

            ui.painter().rect(
                square_rect,
                sow_ui_kit::theme::radius::SM,
                egui::Color32::TRANSPARENT,
                card.stroke,
                egui::StrokeKind::Inside,
            );

            let icon_rect = square_rect;
            if !crate::widgets::try_paint_emoji(ui.painter(), &emoji, icon_rect, tint) {
                ui.painter().text(
                    icon_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &emoji,
                    egui::FontId::proportional(btn_size),
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
                    &hotkey,
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

            let text_color = if !can_afford {
                egui::Color32::from_rgb(239, 68, 68)
            } else if is_selected {
                accent
            } else {
                egui::Color32::from_rgb(230, 230, 230)
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
                if is_nuke {
                    let kind = match item { StripItem::Nuke(k) => *k, _ => unreachable!() };
                    if is_selected {
                        state.selected_nuke_kind = None;
                    } else {
                        state.selected_nuke_kind = Some(kind);
                        state.selected_building_kind = None;
                    }
                } else {
                    let kind = match item { StripItem::Building(k) => *k, _ => unreachable!() };
                    if is_selected {
                        state.selected_building_kind = None;
                    } else {
                        state.selected_building_kind = Some(kind);
                        state.selected_nuke_kind = None;
                    }
                }
            }
        }
    });
}

pub(in crate::ui::hud) fn tab_accent(tab: BottomHudTab) -> Color32 {
    match tab {
        BottomHudTab::Controls => sow_ui_kit::theme::palette::neon_cyan(),
        BottomHudTab::BattleLog => sow_ui_kit::theme::palette::danger(),
        BottomHudTab::EventLog => sow_ui_kit::theme::palette::neon_gold_hover(),
    }
}

pub(in crate::ui::hud) fn draw_browser_tab_strip(
    ui: &mut egui::Ui,
    state: &mut HudState,
    compact: bool,
    dispatch_total: usize,
    event_unread: usize,
    _asset_loader: &crate::ui::asset_loader::AssetLoader,
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
        ui.spacing_mut().item_spacing.x = sow_ui_kit::theme::tab::GAP;

        let tabs = [
            (BottomHudTab::Controls, 0_usize),
            (BottomHudTab::BattleLog, dispatch_unread),
            (BottomHudTab::EventLog, event_unread),
        ];

        for (tab, badge) in tabs {
            let selected = state.bottom_tab == tab;
            let resp = sow_ui_kit::theme::draw_tab(
                ui,
                match tab {
                    BottomHudTab::Controls => sow_ui_kit::theme::TabContent::Text("🛠️"),
                    BottomHudTab::BattleLog => sow_ui_kit::theme::TabContent::Text("📜"),
                    BottomHudTab::EventLog => sow_ui_kit::theme::TabContent::Text("📋"),
                },
                selected,
                tab_accent(tab),
                badge,
                tab_w,
                compact,
            );
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

    sow_ui_kit::theme::draw_tab_baseline(ui, strip_response.response.rect, active_rect);
    active_rect
}
pub(in crate::ui::hud) fn hud_sidebar_row_height(
    compact: bool,
    spawn_active: bool,
    dialog_active: bool,
    main: HudSidebarMain,
    main_w: f32,
    chrome_scale: f32,
) -> f32 {
    let s = if compact { chrome_scale } else { 1.0 };
    if dialog_active {
        return if compact { 240.0 * s } else { 160.0 };
    }
    if spawn_active {
        return if compact { 72.0 * s } else { 56.0 };
    }
    let header_h = if compact { 24.0 * s } else { 22.0 };
    let row_gap = sow_ui_kit::theme::margin::TIGHT as f32;
    let body_h = match main {
        HudSidebarMain::Controls => {
            let extra = if sow_core::config::ENABLE_MISSILE_STRUCTURES { 1.0 } else { 0.0 };
            let num_items = 4.0 + extra;
            let spacing = if compact { 4.0 } else { 12.0 };
            let col_w = (main_w - (num_items - 1.0) * spacing) / num_items;
            let btn_size = col_w.min(if compact { 40.0 * s } else { 48.0 });
            let gold_plate_h = if compact { 14.0 * s } else { 18.0 };
            btn_size + 4.0 + gold_plate_h
        }
        HudSidebarMain::BattleLog | HudSidebarMain::EventLog => {
            if compact {
                120.0 * s
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

#[allow(clippy::too_many_arguments)]
pub(in crate::ui::hud) fn draw_hud_sidebar_row(
    ui: &mut egui::Ui,
    state: &mut HudState,
    content_w: f32,
    compact: bool,
    _action: &mut Option<UiAction>,
    lang: Language,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    main: HudSidebarMain,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
) {
    let spawn_active = state.spawn_timer_secs.is_some();
    let dialog_active = state.bottom_dialog.is_some();
    let row_gap = sow_ui_kit::theme::margin::TIGHT as f32;
    let chrome_scale = if compact {
        sow_ui_kit::theme::viewport_scale(ui.ctx())
    } else {
        1.0
    };
    let row_h = hud_sidebar_row_height(compact, spawn_active, dialog_active, main, content_w, chrome_scale);

    ui.allocate_ui_with_layout(
        vec2(content_w, row_h),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.allocate_ui_with_layout(
                vec2(content_w, row_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.spacing_mut().item_spacing.y = row_gap;
                    if let Some(dlg) = state.bottom_dialog.clone() {
                        let clicked = crate::widgets::paint_dialog_contents(
                            ui,
                            dlg.visual.as_ref(),
                            dlg.name.as_deref(),
                            &dlg.title,
                            &dlg.body,
                            &dlg.buttons,
                            asset_loader,
                            compact,
                        );
                        let blocker = ui.interact(
                            ui.max_rect(),
                            egui::Id::new("hud_bottom_dialog_block"),
                            egui::Sense::click(),
                        );
                        let mut click = clicked;
                        if dlg.click_anywhere {
                            if click.is_none() && blocker.clicked() {
                                click = Some(0);
                            }
                            if blocker.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                        }
                        if let Some(secs) = dlg.auto_dismiss_secs {
                            let started_id = egui::Id::new("hud_pinned_dialog_started_at");
                            let started_for = egui::Id::new("hud_pinned_dialog_started_for");
                            let now = ui.ctx().input(|i| i.time);
                            let same = ui.ctx().data(|d| d.get_temp::<String>(started_for)).as_deref() == Some(dlg.id.as_str());
                            let started = if same {
                                ui.ctx().data(|d| d.get_temp::<f64>(started_id)).unwrap_or(now)
                            } else {
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(started_for, dlg.id.clone());
                                    d.insert_temp(started_id, now);
                                });
                                now
                            };
                            if (now - started) as f32 >= secs {
                                click = click.or(Some(0));
                            } else {
                                ui.ctx().request_repaint();
                            }
                        }
                        if let Some(idx) = click {
                            state.bottom_dialog_click = Some((dlg.id.clone(), idx));
                        }
                    } else {
                        if !spawn_active {
                            match main {
                                HudSidebarMain::Controls => {
                                    ui.push_id("controls_tab", |ui| {
                                        draw_controls_tab(
                                            ui,
                                            state,
                                            content_w,
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
                                            content_w,
                                            compact,
                                            cancel_intents,
                                            lang,
                                        );
                                    });
                                }
                                HudSidebarMain::EventLog => {
                                    ui.push_id("event_log_tab", |ui| {
                                        event_log::draw_event_log_tab(ui, state, content_w, compact, lang);
                                    });
                                }
                            }
                        }
                        ui.push_id("persistent_header", |ui| {
                            troop_spawn::draw_persistent_header(ui, state, compact, lang);
                        });
                    }
                },
            );
        },
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::ui::hud) fn draw_controls_row(
    ui: &mut egui::Ui,
    state: &mut HudState,
    content_w: f32,
    compact: bool,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    lang: Language,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
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
        asset_loader,
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
            ui.add_space(sow_ui_kit::theme::margin::COZY as f32);
        }
        mobile::draw_mobile_selection_bar(ui, state, cancel_intents, lang);
    }
}
