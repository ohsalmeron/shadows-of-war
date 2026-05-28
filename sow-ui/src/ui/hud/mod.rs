use crate::UiAction;
use egui::{pos2, vec2, Align2, Color32, Context, RichText, Slider, Stroke};
use sow_core::protocol::{AttackSnapshot, FleetSnapshot, PlayerSnapshot};
use sow_lang::Language;
use web_time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct NukeAlertDisplay {
    pub message: String,
    pub color: Color32,
    pub spawned_at: Instant,
}



#[derive(Clone, Debug)]
pub struct SelectedTileInfo {
    pub tile_idx: u32,
    pub owner_id: u16,
    pub is_own_territory: bool,
    pub is_friendly: bool,
    pub is_spawning: bool,
    pub is_land: bool,
}

pub struct HudState {
    pub gold: f64,
    pub troops: f64,
    pub max_troops: f64,
    pub attack_ratio: f32,
    pub spawn_timer_secs: Option<f32>,
    pub sync_state: Option<sow_core::protocol::ServerSyncStateMessage>,

    pub my_player_id: u16,
    pub map_w: u32,
    pub attacks: Vec<AttackSnapshot>,
    pub fleets: Vec<FleetSnapshot>,
    pub players: Vec<PlayerSnapshot>,
    pub safe_area_top: f32,
    pub safe_area_bottom: f32,
    pub selected_tile: Option<SelectedTileInfo>,
    pub show_emoji_panel: bool,
    pub emoji_panel_pos: Option<egui::Pos2>,
    pub emoji_panel_just_opened: bool,
    pub pin_emoji: bool,
    pub show_alliance_inbox: bool,
    pub(crate) prev_requests: Vec<u16>,
    pub(crate) last_request_time: Option<Instant>,
    pub show_betrayal_warning: Option<(u16, sow_core::protocol::GameplayIntent)>,
    pub show_error: Option<String>,
    pub(crate) last_error_message: Option<String>,
    pub(crate) error_display_timer: Option<Instant>,
    pub selected_building_kind: Option<sow_core::game::BuildingKind>,
    pub building_costs: [f64; 9],
    pub selected_nuke_kind: Option<sow_core::game::NukeKind>,
    pub nuke_alerts: Vec<NukeAlertDisplay>,
    pub show_attacks_panel: bool,
    pub gold_gain: Option<f64>,
    pub gold_gain_at: Option<Instant>,
    pub prev_gold: f64,
    pub show_ask_panel: Option<u16>,
    pub ask_gold: f64,
    pub ask_troops: f64,
    pub prev_resource_requests: Vec<u16>,
}

impl HudState {
    pub fn push_notification(&mut self, message: String, color: Color32) {
        self.nuke_alerts.push(NukeAlertDisplay {
            message,
            color,
            spawned_at: Instant::now(),
        });
    }
}

fn draw_buildings_dock_no_frame(
    ui: &mut egui::Ui,
    state: &mut HudState,
    width: f32,
    compact: bool,
) {
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

        for (display_idx, &kind) in active_kinds.iter().enumerate() {
            let cost_idx = sow_core::game::BuildingKind::ALL.iter().position(|&k| k == kind).unwrap_or(0);
            let cost = state.building_costs[cost_idx];
            let is_selected = state.selected_building_kind == Some(kind);
            let can_afford = state.gold >= cost;

            let tint = if is_selected {
                crate::ui::theme::accent_solo_cyan()
            } else if !can_afford {
                egui::Color32::from_rgb(180, 50, 50)
            } else {
                egui::Color32::WHITE
            };

            let bg_color = if is_selected {
                crate::ui::theme::accent_solo_cyan().linear_multiply(0.15)
            } else {
                egui::Color32::from_rgba_unmultiplied(10, 15, 25, 120)
            };

            let stroke = if is_selected {
                egui::Stroke::new(1.5_f32, crate::ui::theme::accent_solo_cyan())
            } else if !can_afford {
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(180, 50, 50))
            } else {
                egui::Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border().linear_multiply(0.5))
            };

            let (rect, mut resp) = ui.allocate_exact_size(
                egui::vec2(col_w, if compact { 38.0 } else { 44.0 }),
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

                ui.label(egui::RichText::new(name).strong().size(14.0).color(crate::ui::theme::accent_solo_cyan()));
                ui.add_space(4.0);
                ui.label(egui::RichText::new(desc).size(12.0).color(egui::Color32::LIGHT_GRAY));
                ui.add_space(6.0);

                let cost_text = if cost.is_infinite() { "N/A".to_string() } else { crate::utils::format_number(cost) };
                let cost_color = if can_afford { egui::Color32::from_rgb(74, 222, 128) } else { egui::Color32::from_rgb(239, 68, 68) };
                ui.label(egui::RichText::new(format!("Cost: 🪙 {} Gold", cost_text)).strong().size(13.0).color(cost_color));
            });

            let is_hovered = resp.hovered();
            let final_bg = if is_hovered && !is_selected {
                crate::ui::theme::nickname_field_bg().linear_multiply(0.3)
            } else {
                bg_color
            };

            ui.painter().rect(rect, 6, final_bg, stroke, egui::StrokeKind::Inside);

            // Hotkey badge (top-left corner)
            if !compact {
                let hotkey_color = if is_selected {
                    crate::ui::theme::accent_solo_cyan()
                } else {
                    egui::Color32::from_white_alpha(120)
                };
                ui.painter().text(
                    egui::pos2(rect.left() + 5.0, rect.top() + 5.0),
                    egui::Align2::LEFT_TOP,
                    format!("{}", display_idx + 1),
                    egui::FontId::proportional(7.0),
                    hotkey_color,
                );
            }

            let icon_size = if compact { 16.0 } else { 22.0 };
            let icon_rect = egui::Rect::from_center_size(
                egui::pos2(rect.center().x, rect.top() + (if compact { 11.0 } else { 14.0 })),
                egui::vec2(icon_size, icon_size),
            );

            let uri = kind.asset().uri();

            let image = egui::Image::new(uri).tint(tint);
            image.paint_at(ui, icon_rect);

            let cost_text = if cost.is_infinite() {
                "N/A".to_string()
            } else {
                crate::utils::format_number(cost)
            };

            let text_color = if !can_afford {
                egui::Color32::from_rgb(239, 68, 68)
            } else if is_selected {
                crate::ui::theme::accent_solo_cyan()
            } else {
                egui::Color32::GRAY
            };

            let label_text = if compact {
                cost_text
            } else {
                format!("{} - {}", kind.as_str(), cost_text)
            };

            let font_size = if compact { 8.0 } else { 9.0 };
            ui.painter().text(
                egui::pos2(rect.center().x, rect.bottom() - (if compact { 5.0 } else { 8.0 })),
                egui::Align2::CENTER_CENTER,
                label_text,
                egui::FontId::proportional(font_size),
                text_color,
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
                let uri = nuke_kind.asset().uri();
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
                    egui::Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border().linear_multiply(0.5))
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
                    crate::ui::theme::nickname_field_bg().linear_multiply(0.3)
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
                let image = egui::Image::new(uri).tint(tint);
                image.paint_at(ui, icon_rect);

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

pub fn draw(
    ui: &mut egui::Ui,
    state: &mut HudState,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    lang: Language,
) -> Option<UiAction> {
    static REGISTER_ONCE: std::sync::Once = std::sync::Once::new();
    REGISTER_ONCE.call_once(|| {
        sow_core::register_game_assets(ui.ctx());
    });

    let mut action = None;

    let rect = ui.ctx().content_rect();
    let compact = rect.width() < 1024.0 || rect.width() < rect.height() * 1.25;

    let panel_w = if compact {
        ui.ctx().content_rect().width() - 84.0
    } else {
        500.0
    };

    let bottom_anchor = if compact {
        egui::Align2::CENTER_BOTTOM
    } else {
        egui::Align2::CENTER_BOTTOM
    };
    let bottom_offset = if compact {
        egui::vec2(0.0, -state.safe_area_bottom)
    } else {
        egui::vec2(0.0, -state.safe_area_bottom)
    };

    egui::Area::new(egui::Id::new("hud_bottom_area_v8"))
        .anchor(bottom_anchor, bottom_offset)
        .order(egui::Order::Foreground)
        .movable(false)
        .show(ui.ctx(), |ui| {
            ui.set_max_width(panel_w);

            let panel_bg = crate::ui::theme::panel_bg_transparent();
            let border_color =
                if state.selected_building_kind.is_some() || state.selected_nuke_kind.is_some() {
                    crate::ui::theme::accent_solo_cyan()
                } else {
                    crate::ui::theme::nickname_field_border().linear_multiply(0.4)
                };

            let frame_margin = if compact {
                egui::Margin::symmetric(12, 10)
            } else {
                egui::Margin::symmetric(10, 8)
            };

            let unified_frame = egui::Frame::NONE
                .fill(panel_bg)
                .stroke(egui::Stroke::new(1.0_f32, border_color))
                .corner_radius(egui::CornerRadius::same(12))
                .inner_margin(frame_margin);

            unified_frame.show(ui, |ui| {
                ui.set_width(panel_w - 20.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = if compact { 14.0 } else { 8.0 };

                    // 2. Attacks Display
                    let my_pid = state.my_player_id;
                    let incoming_count = state
                        .attacks
                        .iter()
                        .filter(|a| a.target_owner == my_pid)
                        .count();
                    let outgoing_count = state
                        .attacks
                        .iter()
                        .filter(|a| a.owner_id == my_pid)
                        .count();
                    let fleet_count = state.fleets.iter().filter(|f| f.owner_id == my_pid).count();
                    let total_attacks = incoming_count + outgoing_count + fleet_count;

                    if !compact && my_pid != 0 && total_attacks > 0 {
                        // Desktop: always show attacks inline
                        ui.push_id("attacks_display", |ui| {
                            draw_attacks_display(
                                ui,
                                state,
                                panel_w - 36.0,
                                compact,
                                cancel_intents,
                                lang,
                            );
                        });
                        ui.separator();
                    }

                    // 3. Main Gameplay dock (Building Dock)
                    if state.spawn_timer_secs.is_none() {
                        ui.push_id("building_dock", |ui| {
                            draw_buildings_dock_no_frame(ui, state, panel_w - 40.0, compact);
                        });
                        ui.separator();
                    }

                    // 4. Control Panel (Stats, Gold, and Attack ratio slider)
                    ui.push_id("control_panel_frame", |ui| {
                        if let Some(secs) = state.spawn_timer_secs {
                            draw_spawn_panel(ui, secs, compact, lang);
                        } else {
                            draw_control_panel(ui, state, compact);
                        }
                    });

                    // 5. Mobile selection/actions
                    if compact {
                        draw_mobile_selection_bar(ui, state, cancel_intents, lang);
                    }

                    // Safe area padding
                    if state.safe_area_bottom > 0.0 {
                        ui.add_space(state.safe_area_bottom);
                    }
                });
            });
        });

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

    egui::Area::new(egui::Id::new("hud_exit_button"))
        .anchor(Align2::RIGHT_TOP, vec2(-12.0, 12.0 + state.safe_area_top))
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            crate::ui::theme::hud_panel_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    let btn_resp = ui
                        .add(crate::widgets::HudButton::new("📩"))
                        .on_hover_text(&sow_lang::get(lang).hud.inbox_title);
                    if btn_resp.clicked() {
                        state.show_alliance_inbox = !state.show_alliance_inbox;
                    }

                    // Render red badge with active notifications at all times (bounces/pops on new requests)
                    if total_notifications > 0 {
                        let mut scale = 1.0_f32;
                        if let Some(t) = state.last_request_time {
                            let elapsed = t.elapsed().as_secs_f32();
                            if elapsed < 0.6_f32 {
                                let progress = elapsed / 0.6_f32;
                                // Elastic bounce: pops up quickly, wobbles, and settles back to 1.0
                                scale = 1.0_f32
                                    + 0.8_f32
                                        * (progress * std::f32::consts::PI).sin()
                                        * (1.0_f32 - progress);
                                ui.ctx().request_repaint(); // keep animating
                            }
                        }

                        let badge_center = btn_resp.rect.right_top() + egui::vec2(-2.0, 2.0);
                        let badge_radius = 8.0_f32 * scale;
                        ui.painter().circle_filled(
                            badge_center,
                            badge_radius,
                            Color32::from_rgb(239, 68, 68),
                        );
                        ui.painter().text(
                            badge_center,
                            egui::Align2::CENTER_CENTER,
                            total_notifications.to_string(),
                            egui::FontId::proportional(10.0_f32 * scale),
                            Color32::WHITE,
                        );
                    }

                    if ui
                        .add(crate::widgets::HudButton::new("⚙"))
                        .on_hover_text(&sow_lang::get(lang).hud.hover_settings)
                        .clicked()
                    {
                        action = Some(UiAction::ToggleSettings);
                    }
                    if ui
                        .add(
                            crate::widgets::HudButton::new("✖")
                                .color(Color32::from_rgb(255, 100, 100)),
                        )
                        .on_hover_text(&sow_lang::get(lang).hud.hover_exit)
                        .clicked()
                    {
                        action = Some(UiAction::LeaveLobby);
                    }
                });
            });
        });
    // ── Floating Alliance Inbox Panel ─────────────────────────────────────────
    let is_inbox_active = state.show_alliance_inbox;
    let inbox_progress = ui.ctx().animate_bool_with_time(
        egui::Id::new("alliance_inbox_animation"),
        is_inbox_active,
        0.22,
    );
    if inbox_progress > 0.01 {
        let anim_scale = if is_inbox_active {
            let t = inbox_progress;
            if t >= 1.0 {
                1.0
            } else {
                1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
            }
        } else {
            inbox_progress
        };
        // Slide in horizontally from 320px off-screen on the right to -12px margin
        let x_offset = -12.0 + 320.0 * (1.0 - anim_scale);

        egui::Area::new(egui::Id::new("floating_alliance_inbox"))
            .anchor(Align2::RIGHT_TOP, vec2(x_offset, 56.0 + state.safe_area_top))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                ui.set_max_width(300.0);
                ui.vertical(|ui| {
                    let frame_res = egui::Frame::menu(&ui.ctx().global_style())
                        .fill(crate::ui::theme::panel_bg())
                        .stroke(egui::Stroke::new(1.5_f32, crate::ui::theme::accent_solo_cyan().linear_multiply(inbox_progress)))
                        .corner_radius(12)
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
                                ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);

                                // Title "ALLIANCES"
                                crate::ui::theme::outlined_label(
                                    ui,
                                    &sow_lang::get(lang).hud.inbox_title,
                                    egui::FontId::proportional(12.0),
                                    crate::ui::theme::accent_solo_cyan().linear_multiply(inbox_progress),
                                );
                                ui.add_space(4.0);

                                // Reject All / Accept All — only when 2+ requests
                                if requests.len() > 1 {
                                    let w = (ui.available_width() - 6.0) / 2.0;
                                    ui.horizontal(|ui| {
                                        if ui.add_sized(egui::vec2(w, 24.0),
                                            crate::widgets::ThemeButton::new("REJECT ALL")
                                                .text_size(10.0)
                                                .custom_fill(crate::ui::theme::menu_secondary_button())
                                                .custom_text_color(Color32::from_rgb(239, 68, 68).linear_multiply(inbox_progress))
                                        ).clicked() {
                                            for &req in &requests {
                                                cancel_intents.push(sow_core::protocol::GameplayIntent::RejectAlliance { target_player: req });
                                            }
                                            state.show_alliance_inbox = false;
                                        }
                                        if ui.add_sized(egui::vec2(w, 24.0),
                                            crate::widgets::ThemeButton::new("ACCEPT ALL")
                                                .text_size(10.0)
                                                .custom_fill(crate::ui::theme::menu_secondary_button())
                                                .custom_text_color(Color32::from_rgb(74, 222, 128).linear_multiply(inbox_progress))
                                        ).clicked() {
                                            for &req in &requests {
                                                cancel_intents.push(sow_core::protocol::GameplayIntent::AcceptAlliance { target_player: req });
                                            }
                                            state.show_alliance_inbox = false;
                                        }
                                    });
                                    ui.add_space(2.0);
                                }

                                // Request cards
                                if requests.is_empty() && resource_requests.is_empty() {
                                    crate::ui::theme::outlined_label(
                                        ui,
                                        &sow_lang::get(lang).hud.inbox_empty,
                                        egui::FontId::proportional(11.0),
                                        Color32::GRAY.linear_multiply(inbox_progress),
                                    );
                                }
                                for &requester_id in &requests {
                                    let Some(requester) = state.players.iter().find(|p| p.id == requester_id) else { continue };
                                    let is_renewal = my_snapshot
                                        .map(|me| me.alliances.contains(&requester_id))
                                        .unwrap_or(false);
                                    let rgb = if requester.player_type == sow_core::player::PlayerType::Human {
                                        sow_core::player::human_shader_territory_rgb(requester.id)
                                    } else {
                                        requester.color
                                    };
                                    let pc = Color32::from_rgb(
                                        (rgb[0] * 255.0) as u8,
                                        (rgb[1] * 255.0) as u8,
                                        (rgb[2] * 255.0) as u8,
                                    ).linear_multiply(inbox_progress);
                                    let icon = if requester.disconnected { "🔌" } else if requester.id < 200 { "⭐" } else { "🐺" };
                                    let name = if requester.name.is_empty() {
                                        if requester.id >= 200 { format!("Tribe {}", requester.id - 199) }
                                        else { format!("Nation {}", requester.id - 103) }
                                    } else {
                                        requester.name.clone()
                                    };

                                    // Animate individual card sliding horizontally with spring overshoot!
                                    let card_progress = ui.ctx().animate_bool_with_time(egui::Id::new(("request_card", requester_id)), true, 0.22);
                                    let card_scale = 1.0 - (card_progress * 7.5).cos() * (-3.5 * card_progress).exp();
                                    let card_offset = 30.0 * (1.0 - card_scale.max(0.0));

                                    ui.horizontal(|ui| {
                                        if card_offset > 0.01 {
                                            ui.add_space(card_offset);
                                        }

                                        egui::Frame::NONE
                                            .fill(crate::ui::theme::nickname_field_bg().linear_multiply(0.5 * inbox_progress * card_progress))
                                            .stroke(egui::Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border().linear_multiply(0.4 * inbox_progress * card_progress)))
                                            .corner_radius(6)
                                            .inner_margin(egui::Margin::symmetric(8, 6))
                                            .show(ui, |ui| {
                                                ui.vertical(|ui| {
                                                    // Name row
                                                    ui.horizontal(|ui| {
                                                        if icon.contains('⭐') {
                                                            static REGISTER_STAR_ONCE: std::sync::Once = std::sync::Once::new();
                                                            REGISTER_STAR_ONCE.call_once(|| {
                                                                ui.ctx().include_bytes(
                                                                    "bytes://star.svg",
                                                                    include_bytes!("../../../../sow-client/assets/star.svg").as_slice(),
                                                                );
                                                            });
                                                            let star_size = 18.0_f32; // bigger size to compensate for native emoji
                                                            let load_res = ui.ctx().try_load_texture(
                                                                "bytes://star.svg",
                                                                egui::TextureOptions::default(),
                                                                egui::load::SizeHint::Size {
                                                                    width: (star_size * 2.0).round() as u32,
                                                                    height: (star_size * 2.0).round() as u32,
                                                                    maintain_aspect_ratio: true,
                                                                },
                                                            );
                                                            if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
                                                                ui.image((texture.id, egui::vec2(star_size, star_size)));
                                                            } else {
                                                                crate::ui::theme::outlined_label(ui, "⭐", egui::FontId::proportional(14.0), pc);
                                                            }
                                                        } else {
                                                            crate::ui::theme::outlined_label(ui, icon, egui::FontId::proportional(14.0), pc);
                                                        }
                                                        ui.vertical(|ui| {
                                                            ui.spacing_mut().item_spacing.y = 0.0;
                                                            crate::ui::theme::outlined_label(ui, &name, egui::FontId::proportional(12.5), pc);
                                                            let prompt = if is_renewal {
                                                                match lang {
                                                                    sow_lang::Language::Spanish => "¡quiere renovar la alianza!".to_string(),
                                                                    _ => "wants to renew your alliance!".to_string(),
                                                                }
                                                            } else {
                                                                sow_lang::get(lang).hud.inbox_wants_ally.clone()
                                                            };
                                                            crate::ui::theme::outlined_label(ui, &prompt, egui::FontId::proportional(10.5), Color32::LIGHT_GRAY.linear_multiply(inbox_progress * card_progress));
                                                        });
                                                    });
                                                    ui.add_space(2.0);
                                                    // Button row
                                                    let bw = (ui.available_width() - 6.0) / 2.0;
                                                    let is_last = total_notifications == 1;
                                                    ui.horizontal(|ui| {
                                                        if ui.add_sized(egui::vec2(bw, 24.0),
                                                            crate::widgets::ThemeButton::new(&sow_lang::get(lang).hud.btn_accept)
                                                                .text_size(11.0)
                                                                .custom_fill(crate::ui::theme::menu_secondary_button())
                                                                .custom_text_color(Color32::from_rgb(74, 222, 128).linear_multiply(inbox_progress))
                                                        ).clicked() {
                                                            cancel_intents.push(sow_core::protocol::GameplayIntent::AcceptAlliance { target_player: requester.id });
                                                            if is_last { state.show_alliance_inbox = false; }
                                                        }
                                                        if ui.add_sized(egui::vec2(bw, 24.0),
                                                            crate::widgets::ThemeButton::new(&sow_lang::get(lang).hud.btn_reject)
                                                                .text_size(11.0)
                                                                .custom_fill(crate::ui::theme::menu_secondary_button())
                                                                .custom_text_color(Color32::from_rgb(239, 68, 68).linear_multiply(inbox_progress))
                                                        ).clicked() {
                                                            cancel_intents.push(sow_core::protocol::GameplayIntent::RejectAlliance { target_player: requester.id });
                                                            if is_last { state.show_alliance_inbox = false; }
                                                        }
                                                    });
                                                });
                                            });
                                    });
                                    ui.add_space(2.0);
                                }

                                for req in &resource_requests {
                                    let requester_id = req.requester;
                                    let Some(requester) = state.players.iter().find(|p| p.id == requester_id) else { continue };
                                    let rgb = if requester.player_type == sow_core::player::PlayerType::Human {
                                        sow_core::player::human_shader_territory_rgb(requester.id)
                                    } else {
                                        requester.color
                                    };
                                    let pc = Color32::from_rgb(
                                        (rgb[0] * 255.0) as u8,
                                        (rgb[1] * 255.0) as u8,
                                        (rgb[2] * 255.0) as u8,
                                    ).linear_multiply(inbox_progress);
                                    let icon = if requester.disconnected { "🔌" } else if requester.id < 200 { "⭐" } else { "🐺" };
                                    let name = if requester.name.is_empty() {
                                        if requester.id >= 200 { format!("Tribe {}", requester.id - 199) }
                                        else { format!("Nation {}", requester.id - 103) }
                                    } else {
                                        requester.name.clone()
                                    };

                                    let card_progress = ui.ctx().animate_bool_with_time(egui::Id::new(("res_request_card", requester_id)), true, 0.22);
                                    let card_scale = 1.0 - (card_progress * 7.5).cos() * (-3.5 * card_progress).exp();
                                    let card_offset = 30.0 * (1.0 - card_scale.max(0.0));

                                    ui.horizontal(|ui| {
                                        if card_offset > 0.01 {
                                            ui.add_space(card_offset);
                                        }

                                        egui::Frame::NONE
                                            .fill(crate::ui::theme::nickname_field_bg().linear_multiply(0.5 * inbox_progress * card_progress))
                                            .stroke(egui::Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border().linear_multiply(0.4 * inbox_progress * card_progress)))
                                            .corner_radius(6)
                                            .inner_margin(egui::Margin::symmetric(8, 6))
                                            .show(ui, |ui| {
                                                ui.vertical(|ui| {
                                                    // Name row
                                                    ui.horizontal(|ui| {
                                                        crate::ui::theme::outlined_label(ui, icon, egui::FontId::proportional(14.0), pc);
                                                        ui.vertical(|ui| {
                                                            ui.spacing_mut().item_spacing.y = 0.0;
                                                            crate::ui::theme::outlined_label(ui, &name, egui::FontId::proportional(12.5), pc);
                                                            let prompt = match (req.gold > 0.0, req.troops > 0.0) {
                                                                (true, true) => format!("asks for 💰{} & 🛡️{}", crate::utils::format_number(req.gold), crate::utils::format_number(req.troops)),
                                                                (true, false) => format!("asks for 💰{}", crate::utils::format_number(req.gold)),
                                                                (false, true) => format!("asks for 🛡️{}", crate::utils::format_number(req.troops)),
                                                                _ => "asks for resources".to_string(),
                                                            };
                                                            crate::ui::theme::outlined_label(ui, &prompt, egui::FontId::proportional(10.5), Color32::LIGHT_GRAY.linear_multiply(inbox_progress * card_progress));
                                                        });
                                                    });
                                                    ui.add_space(2.0);
                                                    // Button row
                                                    let bw = (ui.available_width() - 6.0) / 2.0;
                                                    let is_last = total_notifications == 1;
                                                    ui.horizontal(|ui| {
                                                        if ui.add_sized(egui::vec2(bw, 24.0),
                                                            crate::widgets::ThemeButton::new(&sow_lang::get(lang).hud.btn_accept)
                                                                .text_size(11.0)
                                                                .custom_fill(crate::ui::theme::menu_secondary_button())
                                                                .custom_text_color(Color32::from_rgb(74, 222, 128).linear_multiply(inbox_progress))
                                                        ).clicked() {
                                                            cancel_intents.push(sow_core::protocol::GameplayIntent::AcceptResourceRequest { target_player: requester.id });
                                                            if is_last { state.show_alliance_inbox = false; }
                                                        }
                                                        if ui.add_sized(egui::vec2(bw, 24.0),
                                                            crate::widgets::ThemeButton::new(&sow_lang::get(lang).hud.btn_reject)
                                                                .text_size(11.0)
                                                                .custom_fill(crate::ui::theme::menu_secondary_button())
                                                                .custom_text_color(Color32::from_rgb(239, 68, 68).linear_multiply(inbox_progress))
                                                        ).clicked() {
                                                            cancel_intents.push(sow_core::protocol::GameplayIntent::RejectResourceRequest { target_player: requester.id });
                                                            if is_last { state.show_alliance_inbox = false; }
                                                        }
                                                    });
                                                });
                                            });
                                    });
                                    ui.add_space(2.0);
                                }
                            });
                        });
                    let response_rect = frame_res.response.rect;
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("alliance_inbox_rect"), response_rect));
                });
            });

        // Click outside the inbox panel closes it
        if ui.ctx().input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ui
                .ctx()
                .input(|i| i.pointer.press_origin().or(i.pointer.interact_pos()))
            {
                let mut click_absorbed = false;
                if let Some(rect) = ui
                    .ctx()
                    .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("alliance_inbox_rect")))
                {
                    if rect.contains(pos) {
                        click_absorbed = true;
                    }
                }

                // Allow clicking the 📩 mailbox button to toggle without auto-closing
                let screen_size = ui.ctx().content_rect();
                let toggle_btn_rect = egui::Rect::from_min_max(
                    pos2(screen_size.right() - 120.0, 12.0 + state.safe_area_top),
                    pos2(screen_size.right(), 50.0 + state.safe_area_top),
                );
                if toggle_btn_rect.contains(pos) {
                    click_absorbed = true;
                }

                if !click_absorbed {
                    state.show_alliance_inbox = false;
                }
            }
        }
        ui.ctx().request_repaint();
    }

    // ── Floating Map Controls ──────────────────────────────────────────────
    egui::Area::new(egui::Id::new("hud_map_controls"))
        .anchor(
            Align2::RIGHT_BOTTOM,
            vec2(-12.0, -100.0 - state.safe_area_bottom),
        )
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            crate::ui::theme::hud_panel_frame().show(ui, |ui| {
                let btn_w = if cfg!(target_os = "android") {
                    48.0
                } else {
                    32.0
                };
                ui.set_width(btn_w);
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.vertical_centered(|ui| {
                    if ui
                        .add(crate::widgets::HudButton::new("+"))
                        .on_hover_text(&sow_lang::get(lang).hud.hover_zoom_in)
                        .clicked()
                    {
                        action = Some(UiAction::ZoomIn);
                    }
                    if ui
                        .add(crate::widgets::HudButton::new("-"))
                        .on_hover_text(&sow_lang::get(lang).hud.hover_zoom_out)
                        .clicked()
                    {
                        action = Some(UiAction::ZoomOut);
                    }
                    if ui
                        .add(crate::widgets::HudButton::new("⌖"))
                        .on_hover_text(&sow_lang::get(lang).hud.hover_center_camera)
                        .clicked()
                    {
                        action = Some(UiAction::CenterCamera);
                    }
                    if ui
                        .add(crate::widgets::HudButton::new("😀"))
                        .on_hover_text("Express Emoji")
                        .clicked()
                    {
                        state.show_emoji_panel = !state.show_emoji_panel;
                        if state.show_emoji_panel {
                            state.emoji_panel_pos = None;
                            state.emoji_panel_just_opened = true;
                        }
                    }
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
                        .add(crate::widgets::HudButton::new("⚔"))
                        .on_hover_text("Active Dispatches");
                    if attacks_btn.clicked() {
                        state.show_attacks_panel = !state.show_attacks_panel;
                    }

                    if total_attacks > 0 {
                        let badge_center = attacks_btn.rect.right_top() + egui::vec2(-2.0, 2.0);
                        ui.painter().circle_filled(
                            badge_center,
                            6.5,
                            Color32::from_rgb(239, 68, 68),
                        );
                        ui.painter().text(
                            badge_center,
                            egui::Align2::CENTER_CENTER,
                            total_attacks.to_string(),
                            egui::FontId::proportional(8.5),
                            Color32::WHITE,
                        );
                    }
                });
            });
        });

    // ── Bottom-left Attack Ratio Panel ────────────────────────────────────
    if state.spawn_timer_secs.is_none() {
        let y_offset = if compact {
            -10.0 - state.safe_area_bottom
        } else {
            -100.0 - state.safe_area_bottom
        };
        egui::Area::new(egui::Id::new("hud_attack_ratio_panel"))
            .anchor(
                Align2::LEFT_BOTTOM,
                vec2(12.0, y_offset),
            )
            .order(egui::Order::Foreground)
            .movable(false)
            .show(ui.ctx(), |ui| {
                crate::ui::theme::hud_panel_frame().show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    let ratio_troops = (state.troops * (state.attack_ratio as f64)).max(0.0);

                    // Vertical layout: label → slider → troops count
                    ui.vertical_centered(|ui| {
                        // Percentage label
                        crate::ui::theme::outlined_label(
                            ui,
                            &format!("{:.0}%", state.attack_ratio * 100.0),
                            egui::FontId::proportional(13.0),
                            egui::Color32::from_rgb(0, 220, 255),
                        );

                        // Vertical slider
                        let mut ratio = state.attack_ratio;
                        let slider = Slider::new(&mut ratio, 0.01..=1.0)
                            .show_value(false)
                            .vertical();
                        if ui.add_sized(vec2(24.0, 120.0), slider).changed() {
                            action = Some(UiAction::SetAttackRatio(ratio));
                        }

                        // Troops count
                        crate::ui::theme::outlined_label(
                            ui,
                            &crate::utils::format_number(ratio_troops),
                            egui::FontId::proportional(11.0),
                            Color32::from_rgb(220, 230, 220),
                        );
                    });
                });
            });
    }

    let is_attacks_active = state.show_attacks_panel;
    let attacks_progress = ui.ctx().animate_bool_with_time(
        egui::Id::new("attacks_panel_animation"),
        is_attacks_active,
        0.22,
    );
    if attacks_progress > 0.01 {
        let anim_scale = if is_attacks_active {
            let t = attacks_progress;
            if t >= 1.0 {
                1.0
            } else {
                1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
            }
        } else {
            attacks_progress
        };

        let screen_rect = ui.ctx().content_rect();
        if compact {
            ui.ctx()
                .layer_painter(egui::LayerId::new(
                    egui::Order::Background,
                    egui::Id::new("attacks_dim_bg"),
                ))
                .rect_filled(
                    screen_rect,
                    0.0,
                    Color32::from_black_alpha((150.0 * attacks_progress) as u8),
                );
        }

        let mut area =
            egui::Area::new(egui::Id::new("floating_attacks_panel")).order(egui::Order::Foreground);

        let y_offset = 120.0 * (1.0 - anim_scale);
        if compact {
            let pos = screen_rect.center() + vec2(0.0, y_offset);
            area = area.pivot(Align2::CENTER_CENTER).fixed_pos(pos);
        } else {
            area = area.anchor(
                Align2::RIGHT_BOTTOM,
                vec2(-64.0, -140.0 - state.safe_area_bottom + y_offset),
            );
        }

        let border_glow = crate::ui::theme::accent_danger().linear_multiply(attacks_progress);

        area.show(ui.ctx(), |ui| {
            let panel_w = if compact {
                (screen_rect.width() - 32.0).min(340.0)
            } else {
                320.0
            };
            ui.set_max_width(panel_w);

            egui::Frame::window(&ui.ctx().global_style())
                .fill(crate::ui::theme::panel_bg().linear_multiply(attacks_progress))
                .stroke(egui::Stroke::new(1.8_f32 * anim_scale, border_glow))
                .shadow(egui::Shadow {
                    blur: if compact { 12 } else { 16 },
                    spread: 0,
                    color: border_glow.linear_multiply(0.25 * attacks_progress),
                    offset: [0, 0],
                })
                .inner_margin(egui::Margin::symmetric(14, 12))
                .corner_radius(12)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 8.0;

                        ui.horizontal(|ui| {
                            crate::ui::theme::outlined_label(
                                ui,
                                "⚔ ACTIVE DISPATCHES",
                                egui::FontId::proportional(14.0),
                                crate::ui::theme::accent_danger_border()
                                    .linear_multiply(attacks_progress),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("✖").clicked() {
                                        state.show_attacks_panel = false;
                                    }
                                },
                            );
                        });
                        ui.separator();

                        draw_attacks_display(
                            ui,
                            state,
                            panel_w - 28.0,
                            compact,
                            cancel_intents,
                            lang,
                        );
                    });
                });
        });

        if ui.ctx().input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ui
                .ctx()
                .input(|i| i.pointer.press_origin().or(i.pointer.interact_pos()))
            {
                let mut click_absorbed = false;
                let screen_size = ui.ctx().content_rect();

                let controls_rect = egui::Rect::from_min_max(
                    pos2(screen_size.right() - 80.0, screen_size.bottom() - 320.0),
                    screen_size.right_bottom(),
                );
                if controls_rect.contains(pos) {
                    click_absorbed = true;
                }

                if compact {
                    let modal_w = (screen_size.width() - 32.0).min(340.0);
                    let modal_h = 240.0;
                    let modal_rect =
                        egui::Rect::from_center_size(screen_size.center(), vec2(modal_w, modal_h));
                    if modal_rect.contains(pos) {
                        click_absorbed = true;
                    }
                } else {
                    let modal_w = 320.0;
                    let modal_h = 240.0;
                    let modal_rect = egui::Rect::from_min_max(
                        pos2(
                            screen_size.right() - modal_w - 76.0,
                            screen_size.bottom() - modal_h - 108.0,
                        ),
                        pos2(screen_size.right() - 64.0, screen_size.bottom() - 100.0),
                    );
                    if modal_rect.contains(pos) {
                        click_absorbed = true;
                    }
                }

                if !click_absorbed {
                    state.show_attacks_panel = false;
                }
            }
        }
        ui.ctx().request_repaint();
    }

    let is_emoji_active = state.show_emoji_panel;
    let emoji_progress = ui.ctx().animate_bool_with_time(
        egui::Id::new("emoji_panel_animation"),
        is_emoji_active,
        0.22,
    );
    if emoji_progress > 0.01 {
        let anim_scale = if is_emoji_active {
            let t = emoji_progress;
            if t >= 1.0 {
                1.0
            } else {
                1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
            }
        } else {
            emoji_progress
        };

        let screen_rect = ui.ctx().content_rect();
        let emojis = &[
            // Row 1: Happy / Expressive
            "😀", "😎", "😏", "😂", "🤣", "😋", "😉", "😜", // Row 2: Wholesome / Love
            "😍", "🥰", "🥳", "🥺", "😇", "🤩", "👍", "❤️",
            // Row 3: Surprised / Confused
            "😮", "🤔", "🧐", "🙄", "🤯", "🤡", "💩", "🤫",
            // Row 4: Anger / Battle-ready
            "😠", "😡", "🤬", "😤", "🥵", "🥶", "🤢", "🤮", // Row 5: Action / Combat
            "⚔️", "🛡️", "🏹", "💣", "💥", "💀", "👑", "💪", // Row 6: Strategy / Status
            "🔥", "👀", "🏳️", "🤝", "💔", "🔌", "⭐", "🐺",
        ];

        if compact {
            // Mobile: Dim background backdrop
            ui.ctx()
                .layer_painter(egui::LayerId::new(
                    egui::Order::Background,
                    egui::Id::new("emoji_dim_bg"),
                ))
                .rect_filled(
                    screen_rect,
                    0.0,
                    Color32::from_black_alpha((150.0 * emoji_progress) as u8),
                );
        }

        let mut area =
            egui::Area::new(egui::Id::new("floating_emoji_panel")).order(egui::Order::Foreground);

        let y_offset = 120.0 * (1.0 - anim_scale);
        if compact {
            let pos = screen_rect.center() + vec2(0.0, y_offset);
            area = area.pivot(Align2::CENTER_CENTER).fixed_pos(pos);
        } else if let Some(pos) = state.emoji_panel_pos {
            let pos = pos + vec2(0.0, y_offset);
            area = area.pivot(Align2::CENTER_CENTER).fixed_pos(pos);
        } else {
            area = area.anchor(
                Align2::RIGHT_BOTTOM,
                vec2(-64.0, -100.0 - state.safe_area_bottom + y_offset),
            );
        }

        let border_glow = Color32::from_rgb(251, 191, 36).linear_multiply(emoji_progress);

        area.show(ui.ctx(), |ui| {
            let frame_res = egui::Frame::window(&ui.ctx().global_style())
                .fill(crate::ui::theme::panel_bg().linear_multiply(emoji_progress))
                .stroke(egui::Stroke::new(1.8_f32 * anim_scale, border_glow))
                .shadow(egui::Shadow {
                    blur: if compact { 12 } else { 16 },
                    spread: 0,
                    color: border_glow.linear_multiply(0.25 * emoji_progress),
                    offset: [0, 0],
                })
                .inner_margin(egui::Margin::same(
                    ((if compact { 10.0 } else { 12.0 }) * anim_scale) as i8,
                ))
                .corner_radius(((if compact { 16.0 } else { 12.0 }) * anim_scale) as u8)
                .show(ui, |ui| {
                    let cols = 8;
                    let btn_size = (if compact { 38.0 } else { 42.0 }) * anim_scale;
                    let emoji_size = (if compact { 26.0 } else { 28.0 }) * anim_scale;
                    let spacing = (if compact { 2.0 } else { 3.0 }) * anim_scale;
                    let grid_width = cols as f32 * btn_size + (cols as f32 - 1.0) * spacing;

                    ui.vertical(|ui| {
                        ui.set_width(grid_width);
                        ui.spacing_mut().item_spacing.y = 0.0;

                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("hmmm")
                                    .strong()
                                    .size(13.0 * anim_scale)
                                    .color(border_glow),
                            );
                        });
                        ui.add_space(8.0 * anim_scale);

                        let mut emoji_idx = 0;
                        for chunk in emojis.chunks(cols) {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = spacing;
                                for &emoji in chunk {
                                    let (rect, resp) = ui.allocate_exact_size(
                                        vec2(btn_size, btn_size),
                                        egui::Sense::click(),
                                    );
                                    let is_hovered = resp.hovered();

                                    let scale_id =
                                        ui.make_persistent_id(("emoji_scale", emoji_idx));
                                    let hover_t =
                                        ui.ctx().animate_bool_with_time(scale_id, is_hovered, 0.15);

                                    // Spring calculation (easeOutBack) - optimized to direct multiplications
                                    let spring_t = if hover_t > 0.0 {
                                        let c1 = 1.70158;
                                        let c3 = c1 + 1.0;
                                        let xm1 = hover_t - 1.0;
                                        1.0 + c3 * xm1 * xm1 * xm1 + c1 * xm1 * xm1
                                    } else {
                                        0.0
                                    };

                                    let bg_color = if is_hovered {
                                        crate::ui::theme::menu_secondary_button_hover()
                                            .linear_multiply((0.6 + 0.3 * hover_t) * emoji_progress)
                                    } else {
                                        crate::ui::theme::nickname_field_bg()
                                            .linear_multiply(0.4 * emoji_progress)
                                    };
                                    let stroke_color =
                                        border_glow.linear_multiply(0.3 + 0.7 * hover_t);
                                    let active_stroke = egui::Stroke::new(
                                        (1.0_f32 + 1.0_f32 * hover_t) * anim_scale,
                                        stroke_color,
                                    );

                                    // Expand rect slightly on hover using spring_t for dynamic size popping
                                    let active_rect = rect.expand(2.0 * spring_t * anim_scale);

                                    ui.painter().rect(
                                        active_rect,
                                        8.0 * anim_scale, // rounded corners radius 8.0
                                        bg_color,
                                        active_stroke,
                                        egui::StrokeKind::Inside,
                                    );

                                    // Font size with spring scaling
                                    let scaled_font_size = emoji_size + 6.0 * spring_t * anim_scale;
                                    if emoji.contains('⭐') {
                                        static REGISTER_STAR_ONCE: std::sync::Once =
                                            std::sync::Once::new();
                                        REGISTER_STAR_ONCE.call_once(|| {
                                            ui.ctx().include_bytes(
                                                "bytes://star.svg",
                                                include_bytes!(
                                                    "../../../../sow-client/assets/star.svg"
                                                )
                                                .as_slice(),
                                            );
                                        });
                                        let star_size = scaled_font_size * 1.25;
                                        let star_rect = egui::Rect::from_center_size(
                                            active_rect.center(),
                                            egui::vec2(star_size, star_size),
                                        );
                                        let size_hint = egui::load::SizeHint::Size {
                                            width: star_size.round() as u32,
                                            height: star_size.round() as u32,
                                            maintain_aspect_ratio: true,
                                        };
                                        let load_res = ui.ctx().try_load_texture(
                                            "bytes://star.svg",
                                            egui::TextureOptions::default(),
                                            size_hint,
                                        );
                                        if let Ok(egui::load::TexturePoll::Ready { texture }) =
                                            load_res
                                        {
                                            ui.painter().image(
                                                texture.id,
                                                star_rect,
                                                egui::Rect::from_min_max(
                                                    egui::pos2(0.0, 0.0),
                                                    egui::pos2(1.0, 1.0),
                                                ),
                                                egui::Color32::WHITE
                                                    .linear_multiply(emoji_progress),
                                            );
                                        } else {
                                            ui.painter().text(
                                                active_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                emoji,
                                                egui::FontId::proportional(scaled_font_size),
                                                Color32::WHITE.linear_multiply(emoji_progress),
                                            );
                                        }
                                    } else {
                                        ui.painter().text(
                                            active_rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            emoji,
                                            egui::FontId::proportional(scaled_font_size),
                                            Color32::WHITE.linear_multiply(emoji_progress),
                                        );
                                    }

                                    emoji_idx += 1;

                                    if resp.clicked() {
                                        let intent =
                                            sow_core::protocol::GameplayIntent::ExpressEmoji {
                                                emoji: emoji.to_owned(),
                                                pinned: state.pin_emoji,
                                            };
                                        cancel_intents.push(intent);
                                        if !state.pin_emoji {
                                            state.show_emoji_panel = false;
                                        }
                                    }
                                }
                            });
                            ui.add_space(spacing);
                        }

                        ui.add_space(8.0 * anim_scale);

                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0 * anim_scale;

                                        let text_color = if state.pin_emoji {
                                            crate::ui::theme::accent_ranked_gold()
                                                .linear_multiply(emoji_progress)
                                        } else {
                                            crate::ui::theme::text_secondary()
                                                .linear_multiply(emoji_progress)
                                        };

                                        let label_resp = ui.label(
                                            RichText::new("PIN")
                                                .size(13.0 * anim_scale)
                                                .strong()
                                                .color(text_color),
                                        );

                                        let label_click = ui.interact(
                                            label_resp.rect,
                                            ui.make_persistent_id("pin_label_click"),
                                            egui::Sense::click(),
                                        );
                                        if label_click.clicked() {
                                            state.pin_emoji = !state.pin_emoji;
                                        }

                                        let box_size = 28.0;
                                        let (rect, resp) = ui.allocate_exact_size(
                                            vec2(box_size, box_size),
                                            egui::Sense::click(),
                                        );

                                        let is_hovered = resp.hovered();
                                        if resp.clicked() {
                                            state.pin_emoji = !state.pin_emoji;
                                        }

                                        let bg_color = if state.pin_emoji {
                                            crate::ui::theme::accent_ranked_gold()
                                                .linear_multiply(0.2)
                                        } else {
                                            crate::ui::theme::nickname_field_bg()
                                        };

                                        let stroke_color = if state.pin_emoji {
                                            crate::ui::theme::accent_ranked_gold()
                                        } else if is_hovered {
                                            crate::ui::theme::accent_solo_cyan()
                                        } else {
                                            crate::ui::theme::nickname_field_border()
                                        };

                                        ui.painter().rect(
                                            rect,
                                            4.0,
                                            bg_color,
                                            egui::Stroke::new(2.0_f32, stroke_color),
                                            egui::StrokeKind::Inside,
                                        );

                                        if state.pin_emoji {
                                            ui.painter().text(
                                                rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                "✓",
                                                egui::FontId::proportional(22.0),
                                                crate::ui::theme::accent_ranked_gold(),
                                            );
                                        }
                                    });
                                },
                            );
                        });
                    });
                });
            let response_rect = frame_res.response.rect;
            ui.ctx()
                .data_mut(|d| d.insert_temp(egui::Id::new("emoji_panel_rect"), response_rect));
        });

        // Robust dynamic Rect-based click-outside dismiss
        if !state.emoji_panel_just_opened && ui.ctx().input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ui
                .ctx()
                .input(|i| i.pointer.press_origin().or(i.pointer.interact_pos()))
            {
                let mut is_outside = true;

                if let Some(rect) = ui
                    .ctx()
                    .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("emoji_panel_rect")))
                {
                    if rect.contains(pos) {
                        is_outside = false;
                    }
                }

                // Allow clicking the HUD bottom bar controls without dismissing if we are in default mode
                if state.emoji_panel_pos.is_none() {
                    let screen_size = ui.ctx().content_rect();
                    let hud_rect = egui::Rect::from_min_max(
                        pos2(
                            screen_size.right() - 510.0,
                            screen_size.bottom() - 90.0 - state.safe_area_bottom,
                        ),
                        pos2(screen_size.right(), screen_size.bottom()),
                    );
                    if hud_rect.contains(pos) {
                        is_outside = false;
                    }
                }

                if is_outside && !state.pin_emoji {
                    state.show_emoji_panel = false;
                }
            }
        }

        state.emoji_panel_just_opened = false;
    }

    draw_transfer_panel(ui, state, cancel_intents);
    draw_sync_overlay(ui.ctx(), state, lang);
    draw_betrayal_overlay(ui.ctx(), state, cancel_intents);
    draw_nuke_alerts(ui.ctx(), state);
    draw_error_overlay(ui.ctx(), state);

    action
}

fn get_player_display_name(players: &[PlayerSnapshot], id: u16, default: &str) -> String {
    players
        .iter()
        .find(|p| p.id == id)
        .map(|p| {
            if p.name.is_empty() {
                if p.id >= 200 {
                    format!("Tribe {}", p.id - 199)
                } else {
                    format!("Nation {}", p.id - 103)
                }
            } else {
                p.name.clone()
            }
        })
        .unwrap_or_else(|| default.to_string())
}

fn draw_attacks_display(
    ui: &mut egui::Ui,
    state: &HudState,
    width: f32,
    compact: bool,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    lang: Language,
) {
    let my_pid = state.my_player_id;
    if my_pid == 0 {
        return;
    }

    let attack_bg = crate::ui::theme::panel_bg_transparent();

    // Count items without allocating
    let incoming_count = state
        .attacks
        .iter()
        .filter(|a| a.target_owner == my_pid)
        .count();
    let outgoing_count = state
        .attacks
        .iter()
        .filter(|a| a.owner_id == my_pid)
        .count();
    let fleet_count = state.fleets.iter().filter(|f| f.owner_id == my_pid).count();
    let total = incoming_count + outgoing_count + fleet_count;

    if total == 0 {
        return;
    }

    // Fixed-height container prevents layout reflow when items change
    let cols = if compact { 1 } else { 2 };
    let rows = total.div_ceil(cols).min(4);
    let row_h = 30.0;
    let fixed_h = rows as f32 * row_h + (rows as f32 - 1.0) * 4.0;
    let cell_w = (width - 16.0 - (cols as f32 - 1.0) * 4.0) / cols as f32;

    let strings = &sow_lang::get(lang).hud;

    egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 0)).show(ui, |ui| {
        egui::ScrollArea::vertical().max_height(fixed_h).stick_to_bottom(true).show(ui, |ui| {
            ui.set_width(width - 16.0);

            let item_count = total;
            let row_count = item_count.div_ceil(cols);

            for r in 0..row_count {
                let data_row = row_count - 1 - r;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for col in 0..cols {
                        let idx = data_row * cols + col;
                        if idx >= item_count { continue; }

                        let glow_color = if idx < incoming_count {
                            crate::ui::theme::accent_danger()
                        } else {
                            crate::ui::theme::accent_solo_cyan()
                        };
                        let glow_alpha = 60;

                        egui::Frame::NONE
                            .fill(attack_bg)
                            .stroke(egui::Stroke::new(1.0_f32, glow_color.linear_multiply(glow_alpha as f32 / 255.0)))
                            .corner_radius(4)
                            .inner_margin(egui::Margin::symmetric(6, 0))
                            .show(ui, |ui| {
                                ui.set_width(cell_w);
                                ui.set_height(row_h);

                                // Right-to-left: button on right, text fills left
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    // 1. Button (fixed 20x20, right side)
                                    if idx < incoming_count {
                                        let attack = state.attacks.iter().filter(|a| a.target_owner == my_pid).nth(idx).unwrap();
                                        if !attack.retreating {
                                            let btn = egui::Button::new(RichText::new("⚔").size(9.0))
                                                .fill(crate::ui::theme::accent_danger().linear_multiply(0.25))
                                                .stroke(egui::Stroke::new(1.0_f32, crate::ui::theme::accent_danger_border()))
                                                .corner_radius(4);
                                            if ui.add_sized(egui::vec2(20.0, 20.0), btn).on_hover_text(&strings.hover_retaliate).clicked() {
                                                let troops = state.troops * (state.attack_ratio as f64);
                                                if troops > 0.0 {
                                                    cancel_intents.push(sow_core::protocol::GameplayIntent::Attack(
                                                        sow_core::protocol::AttackIntent { target_owner: attack.owner_id, troops: Some(troops) }
                                                    ));
                                                }
                                            }
                                        }
                                    } else if idx < incoming_count + outgoing_count {
                                        let attack = state.attacks.iter().filter(|a| a.owner_id == my_pid).nth(idx - incoming_count).unwrap();
                                        if !attack.retreating {
                                            let btn = egui::Button::new(RichText::new("X").size(9.0)).corner_radius(4);
                                            if ui.add_sized(egui::vec2(20.0, 20.0), btn).clicked() {
                                                cancel_intents.push(sow_core::protocol::GameplayIntent::CancelAttack { attack_id: attack.id });
                                            }
                                        }
                                    } else {
                                        let fleet = state.fleets.iter().filter(|f| f.owner_id == my_pid).nth(idx - incoming_count - outgoing_count).unwrap();
                                        if !fleet.retreating {
                                            let btn = egui::Button::new(RichText::new("X").size(9.0)).corner_radius(4);
                                            if ui.add_sized(egui::vec2(20.0, 20.0), btn).clicked() {
                                                cancel_intents.push(sow_core::protocol::GameplayIntent::RecallFleet { fleet_id: fleet.id });
                                            }
                                        }
                                    }

                                    // 2. Text label (fills remaining space, left side)
                                    let (txt, color) = if idx < incoming_count {
                                        let attack = state.attacks.iter().filter(|a| a.target_owner == my_pid).nth(idx).unwrap();
                                        let name: String = get_player_display_name(&state.players, attack.owner_id, &strings.default_player_name).chars().take(10).collect();
                                        (format!("IN {} {}", crate::utils::format_number(attack.troops), name), crate::ui::theme::accent_danger_border())
                                    } else if idx < incoming_count + outgoing_count {
                                        let attack = state.attacks.iter().filter(|a| a.owner_id == my_pid).nth(idx - incoming_count).unwrap();
                                        let name: String = get_player_display_name(&state.players, attack.target_owner, &strings.wilderness_player_name).chars().take(10).collect();
                                        (format!("OUT {} {}", crate::utils::format_number(attack.troops), name), crate::ui::theme::accent_solo_cyan())
                                    } else {
                                        let fleet = state.fleets.iter().filter(|f| f.owner_id == my_pid).nth(idx - incoming_count - outgoing_count).unwrap();
                                        (format!("NAVY {}", crate::utils::format_number(fleet.troops)), crate::ui::theme::accent_solo_cyan())
                                    };
                                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                        ui.label(RichText::new(txt).size(10.0).color(color).strong());
                                    });
                                });
                            });
                     }
                 });
             }
         });
     });
}


#[allow(unused_variables)]
fn draw_control_panel(
    ui: &mut egui::Ui,
    state: &HudState,
    compact: bool,
) {
    let troop_rate = (state.max_troops * 0.1).max(0.0); // Approximation
    let is_increasing = true; // Simplified

    if compact {
        // Mobile Layout: 1 Row [Gold] [Troop Bar (Expanded to full width)]
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;

            // Gold
            let gold_frame_resp = egui::Frame::NONE
                .stroke(Stroke::new(
                    1.0_f32,
                    crate::ui::theme::accent_ranked_gold_hover(),
                ))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(10, 14))
                .show(ui, |ui| {
                    crate::ui::theme::outlined_label(
                        ui,
                        &format!("💰 {}", crate::utils::format_number(state.gold)),
                        egui::FontId::proportional(13.0),
                        crate::ui::theme::accent_ranked_gold_hover(),
                    );
                });

            // Gold gain popup
            if let (Some(amount), Some(at)) = (state.gold_gain, state.gold_gain_at) {
                let t = at.elapsed().as_secs_f32().min(2.5);
                let alpha = ((1.0 - t / 2.5) * 255.0) as u8;
                let slide = 8.0 * (1.0 - (t * 6.0).min(1.0));
                let r = gold_frame_resp.response.rect;
                let text = format!("💰 +{}", crate::utils::format_number(amount));
                let p = pos2(r.center().x, r.top() - 10.0 - slide);
                crate::ui::theme::outlined_text(
                    ui.painter(),
                    p,
                    Align2::CENTER_BOTTOM,
                    &text,
                    egui::FontId::proportional(13.0),
                    Color32::from_rgba_unmultiplied(74, 222, 128, alpha),
                    Color32::from_rgba_unmultiplied(0, 0, 0, alpha),
                );
            }

            // Troop Bar (Takes 100% of the remaining full width!)
            let bar_w = ui.available_width().max(100.0);
            let (rect, _resp) = ui.allocate_exact_size(vec2(bar_w, 48.0), egui::Sense::hover());
            draw_troop_bar(
                ui,
                rect,
                state.troops,
                state.max_troops,
                troop_rate,
                true,
                is_increasing,
            );
        });
    } else {
        // Desktop Layout: [Rate] [Troop Bar] [Gold]
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 8.0;

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                // Rate
                let rate_color = if is_increasing {
                    crate::ui::theme::accent_solo_cyan_hover()
                } else {
                    crate::ui::theme::accent_danger()
                };
                egui::Frame::NONE
                    .stroke(Stroke::new(1.0_f32, rate_color))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(6, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            crate::ui::theme::outlined_label(
                                ui,
                                "⚔",
                                egui::FontId::proportional(14.0),
                                Color32::WHITE,
                            );
                            crate::ui::theme::outlined_label(
                                ui,
                                &format!("+{}/s", crate::utils::format_number(troop_rate)),
                                egui::FontId::proportional(14.0),
                                rate_color,
                            );
                        });
                    });

                // Troop Bar (Flex-1)
                let bar_w = ui.available_width() - 80.0; // Reserve space for gold
                let (rect, _resp) =
                    ui.allocate_exact_size(vec2(bar_w.max(100.0), 24.0), egui::Sense::hover());
                draw_troop_bar(
                    ui,
                    rect,
                    state.troops,
                    state.max_troops,
                    troop_rate,
                    false,
                    is_increasing,
                );

                // Gold
                let gold_frame_resp = egui::Frame::NONE
                    .stroke(Stroke::new(
                        1.0_f32,
                        crate::ui::theme::accent_ranked_gold_hover(),
                    ))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(6, 4))
                    .show(ui, |ui| {
                        crate::ui::theme::outlined_label(
                            ui,
                            &format!("💰 {}", crate::utils::format_number(state.gold)),
                            egui::FontId::proportional(14.0),
                            crate::ui::theme::accent_ranked_gold_hover(),
                        );
                    });

                // Gold gain popup
                if let (Some(amount), Some(at)) = (state.gold_gain, state.gold_gain_at) {
                    let t = at.elapsed().as_secs_f32().min(2.5);
                    let alpha = ((1.0 - t / 2.5) * 255.0) as u8;
                    let slide = 10.0 * (1.0 - (t * 6.0).min(1.0));
                    let r = gold_frame_resp.response.rect;
                    let text = format!("💰 +{}", crate::utils::format_number(amount));
                    let p = pos2(r.center().x, r.top() - 12.0 - slide);
                    crate::ui::theme::outlined_text(
                        ui.painter(),
                        p,
                        Align2::CENTER_BOTTOM,
                        &text,
                        egui::FontId::proportional(16.0),
                        Color32::from_rgba_unmultiplied(74, 222, 128, alpha),
                        Color32::from_rgba_unmultiplied(0, 0, 0, alpha),
                    );
                }
            });
        });
    }
}

fn draw_troop_bar(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    troops: f64,
    max_troops: f64,
    troop_rate: f64,
    compact: bool,
    is_increasing: bool,
) {
    let base = max_troops.max(1.0);
    let green_pct = (troops / base).clamp(0.0, 1.0) as f32;

    // Animate the backfiller so it smoothly catches up to the current troop level
    let catchup_pct = ui.ctx().animate_value_with_time(
        ui.id().with("troop_bar_catchup"),
        green_pct,
        2.0, // Two seconds to drain
    );

    let green_pct_f32 = green_pct;
    // The orange bar (dark green visually) is the gap between the actual troops and the animated catchup
    let orange_pct_f32 = (catchup_pct - green_pct).max(0.0);

    let bg_color = crate::ui::theme::nickname_field_bg();
    let green_color = crate::ui::theme::accent_solo_cyan_hover();
    let orange_color = crate::ui::theme::accent_solo_cyan();

    // Draw background
    ui.painter().rect(
        rect,
        0,
        bg_color,
        Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border()),
        egui::StrokeKind::Inside,
    );

    // Draw green fill
    if green_pct_f32 > 0.0 {
        let green_rect =
            egui::Rect::from_min_size(rect.min, vec2(rect.width() * green_pct_f32, rect.height()));
        ui.painter().rect_filled(green_rect, 0, green_color);
    }

    // Draw orange fill (backfiller)
    if orange_pct_f32 > 0.0 {
        let orange_start = rect.min.x + rect.width() * green_pct_f32;
        let orange_rect = egui::Rect::from_min_size(
            pos2(orange_start, rect.min.y),
            vec2(rect.width() * orange_pct_f32, rect.height()),
        );
        ui.painter().rect_filled(orange_rect, 0, orange_color);
    }

    // Overlay text
    let shadow = Color32::BLACK;
    if compact {
        let troop_text = crate::utils::format_number(troops);
        crate::ui::theme::outlined_text(
            ui.painter(),
            pos2(rect.left() + 6.0, rect.center().y),
            Align2::LEFT_CENTER,
            &troop_text,
            egui::FontId::proportional(12.5),
            Color32::from_rgb(220, 230, 220),
            shadow,
        );
        let max_text = crate::utils::format_number(max_troops);
        crate::ui::theme::outlined_text(
            ui.painter(),
            pos2(rect.right() - 6.0, rect.center().y),
            Align2::RIGHT_CENTER,
            &max_text,
            egui::FontId::proportional(12.5),
            Color32::from_rgb(220, 230, 220),
            shadow,
        );
        let rate_color = if is_increasing {
            crate::ui::theme::accent_solo_cyan_hover()
        } else {
            crate::ui::theme::accent_danger()
        };
        let rate_text = format!("+{}/s", crate::utils::format_number(troop_rate));

        let font_id = egui::FontId::proportional(12.5);
        let galley = ui
            .painter()
            .layout_no_wrap(rate_text.clone(), font_id.clone(), rate_color);
        let icon_size = 11.0;
        let total_w = icon_size + 4.0 + galley.rect.width();
        let mut start_x = rect.center().x - total_w / 2.0;

        ui.painter().text(
            pos2(start_x + icon_size / 2.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            "⚔",
            egui::FontId::proportional(icon_size),
            Color32::WHITE,
        );
        start_x += icon_size + 4.0;

        crate::ui::theme::outlined_text(
            ui.painter(),
            pos2(start_x, rect.center().y),
            Align2::LEFT_CENTER,
            &rate_text,
            font_id,
            rate_color,
            shadow,
        );
    } else {
        let text = format!(
            "{} / {}",
            crate::utils::format_number(troops),
            crate::utils::format_number(max_troops)
        );
        let font_id = egui::FontId::proportional(11.0);
        let galley = ui.painter().layout_no_wrap(
            text.clone(),
            font_id.clone(),
            Color32::from_rgb(220, 230, 220),
        );
        let icon_size = 11.0;
        let total_w = galley.rect.width() + 4.0 + icon_size;
        let mut start_x = rect.center().x - total_w / 2.0;

        crate::ui::theme::outlined_text(
            ui.painter(),
            pos2(start_x, rect.center().y),
            Align2::LEFT_CENTER,
            &text,
            font_id,
            Color32::from_rgb(220, 230, 220),
            shadow,
        );
        start_x += galley.rect.width() + 4.0;

        ui.painter().text(
            pos2(start_x + icon_size / 2.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            "⚔",
            egui::FontId::proportional(icon_size),
            Color32::WHITE,
        );
    }
}

fn draw_spawn_panel(ui: &mut egui::Ui, secs: f32, compact: bool, lang: Language) {
    ui.vertical_centered(|ui| {
        if compact {
            ui.add_space(8.0);
        }
        crate::ui::theme::outlined_label(
            ui,
            &sow_lang::get(lang).hud.spawn_choose_location,
            egui::FontId::proportional(if compact { 16.0 } else { 20.0 }),
            crate::ui::theme::accent_ranked_gold_hover(),
        );
        ui.label(
            RichText::new(format!(
                "{:.1}{}",
                secs,
                sow_lang::get(lang).hud.spawn_seconds_remaining
            ))
            .size(14.0)
            .color(Color32::from_rgb(220, 230, 220)),
        );
        if compact {
            ui.add_space(8.0);
        }
    });
}

fn draw_sync_overlay(ctx: &Context, state: &HudState, lang: Language) {
    if let Some(sync) = &state.sync_state {
        let strings = &sow_lang::get(lang).hud;
        let screen_rect = ctx.content_rect();
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("sync_overlay"),
        ))
        .rect_filled(screen_rect, 0.0, Color32::from_black_alpha(180));

        egui::Window::new(&strings.overlay_waiting_players)
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    if sync.is_starting {
                        crate::ui::theme::outlined_label(
                            ui,
                            &strings.overlay_all_ready,
                            egui::FontId::proportional(24.0),
                            Color32::GREEN,
                        );
                        ui.label(
                            RichText::new(&strings.overlay_stabilizing)
                                .size(16.0)
                                .color(Color32::LIGHT_GRAY),
                        );
                    } else {
                        crate::ui::theme::outlined_label(
                            ui,
                            &strings.overlay_waiting_players,
                            egui::FontId::proportional(24.0),
                            Color32::WHITE,
                        );
                        ui.label(
                            RichText::new(format!(
                                "{}{:.1}{}",
                                strings.overlay_starting_in,
                                sync.time_remaining,
                                strings.overlay_seconds_short
                            ))
                            .size(18.0)
                            .color(Color32::YELLOW),
                        );
                    }

                    ui.add_space(20.0);
                    let total = sync.players.len();
                    let ready = sync.players.iter().filter(|p| p.is_ready).count();
                    let ratio = if total == 0 {
                        0.0
                    } else {
                        ready as f32 / total as f32
                    };
                    ui.add(egui::ProgressBar::new(ratio).text(format!(
                        "{}/{} {}",
                        ready, total, strings.overlay_players_ready
                    )));

                    ui.add_space(15.0);
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for p in &sync.players {
                                ui.horizontal(|ui| {
                                    if p.is_ready {
                                        ui.label(RichText::new("✔").color(Color32::GREEN));
                                    } else {
                                        ui.add(
                                            egui::Spinner::new()
                                                .size(14.0)
                                                .color(Color32::LIGHT_GRAY),
                                        );
                                    }
                                    ui.label(RichText::new(&p.name).color(Color32::WHITE));
                                });
                            }
                        });
                });
            });
    }
}

fn draw_betrayal_overlay(
    ctx: &Context,
    state: &mut HudState,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
) {
    if let Some((ally_id, intent)) = state.show_betrayal_warning.clone() {
        let screen_rect = ctx.content_rect();
        let compact =
            screen_rect.width() < 1024.0 || screen_rect.width() < screen_rect.height() * 1.25;

        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("betrayal_overlay_bg"),
        ))
        .rect_filled(screen_rect, 0.0, Color32::from_black_alpha(180));

        let window = egui::Window::new("betrayal_warning_modal")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .order(egui::Order::Foreground);

        let panel_w = if compact {
            (screen_rect.width() - 32.0).min(400.0)
        } else {
            0.0 // auto-sized on desktop
        };

        let window = if compact {
            window
                .fixed_size(vec2(panel_w, 0.0))
                .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        } else {
            window.anchor(egui::Align2::CENTER_CENTER, vec2(0.0, -20.0))
        };

        window
            .frame(
                egui::Frame::window(&ctx.global_style())
                    .fill(crate::ui::theme::panel_bg())
                    .stroke(egui::Stroke::new(2.0f32, crate::ui::theme::accent_danger()))
                    .inner_margin(if compact { 16.0 } else { 24.0 })
                    .corner_radius(12),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {

                    let ally_name = get_player_display_name(&state.players, ally_id, "Ally");

                    crate::ui::theme::outlined_label(
                        ui,
                        "BETRAYAL WARNING",
                        egui::FontId::proportional(if compact { 22.0 } else { 28.0 }),
                        crate::ui::theme::accent_danger(),
                    );

                    ui.add_space(if compact { 16.0 } else { 12.0 });

                    ui.label(
                        RichText::new(format!(
                            "If you attack {}, other allies could attack you.",
                            ally_name
                        ))
                        .size(if compact { 14.0 } else { 16.0 })
                        .color(Color32::WHITE),
                    );

                    ui.label(
                        RichText::new("Are you sure?")
                            .size(if compact { 15.0 } else { 18.0 })
                            .strong()
                            .color(crate::ui::theme::accent_ranked_gold()),
                    );

                    ui.add_space(if compact { 32.0 } else { 24.0 });

                    let btn_w = if compact {
                        (ui.available_width() - 8.0) / 2.0
                    } else {
                        160.0
                    };
                    let btn_h = if compact { 40.0 } else { 44.0 };

                    ui.horizontal(|ui| {
                        if compact {
                            ui.spacing_mut().item_spacing.x = 8.0;
                        }

                        // NO button (safe)
                        let no_btn = egui::Button::new(
                            RichText::new("NO, KEEP ALLIANCE").size(if compact {
                                13.0
                            } else {
                                16.0
                            }),
                        )
                        .fill(crate::ui::theme::menu_secondary_button())
                        .corner_radius(8);

                        if ui.add_sized(vec2(btn_w, btn_h), no_btn).clicked() {
                            state.show_betrayal_warning = None;
                        }

                        if !compact {
                            ui.add_space(16.0);
                        }

                        // YES button (danger)
                        let yes_btn = egui::Button::new(
                            RichText::new("YES, BETRAY")
                                .size(if compact { 13.0 } else { 16.0 })
                                .strong(),
                        )
                        .fill(crate::ui::theme::accent_danger().linear_multiply(0.3))
                        .stroke(egui::Stroke::new(1.5f32, crate::ui::theme::accent_danger()))
                        .corner_radius(8);

                        if ui
                            .add_sized(vec2(if compact { btn_w } else { 140.0 }, btn_h), yes_btn)
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            // 1. Send BreakAlliance intent
                            cancel_intents.push(
                                sow_core::protocol::GameplayIntent::BreakAlliance {
                                    target_player: ally_id,
                                },
                            );

                            // 2. Send the original Attack intent right after
                            cancel_intents.push(intent);

                            state.show_betrayal_warning = None;
                        }
                    });
                });
            });
    }
}

fn draw_mobile_selection_bar(
    ui: &mut egui::Ui,
    state: &HudState,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    lang: Language,
) {
    if let Some(tile_info) = &state.selected_tile {
        use crate::ui::theme::palette;
        use egui::RichText;
        let strings = &sow_lang::get(lang).hud;

        if tile_info.is_spawning {
            return;
        }

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let text_color = if tile_info.is_own_territory {
                    palette::neon_gold()
                } else if tile_info.is_friendly {
                    palette::neon_cyan()
                } else {
                    palette::danger()
                };
                let status_text = if tile_info.is_own_territory {
                    &strings.status_own
                } else if tile_info.is_friendly {
                    &strings.status_ally
                } else if tile_info.owner_id != 0 {
                    &strings.status_enemy
                } else {
                    &strings.status_neutral
                };
                ui.label(
                    RichText::new(format!(
                        "{}{}-{}",
                        strings.status_tile_prefix, tile_info.tile_idx, status_text
                    ))
                    .strong()
                    .size(11.0)
                    .color(text_color),
                );
            });

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;

                let btn_w = (ui.available_width() - 12.0) / 4.0;
                let btn_h = 48.0;

                // 1. Info Button
                let info_btn =
                    egui::Button::new(RichText::new(&strings.btn_info).strong().size(13.0))
                        .fill(palette::button_inactive())
                        .stroke(egui::Stroke::new(1.0_f32, palette::text_muted()))
                        .corner_radius(6);
                let _ = ui.add_sized(egui::vec2(btn_w, btn_h), info_btn);

                // 2. Fleet / Delete Button
                let right_fill = if tile_info.is_own_territory {
                    palette::danger()
                } else {
                    palette::neon_cyan()
                };
                let right_glow = if tile_info.is_own_territory {
                    palette::danger_border()
                } else {
                    palette::neon_cyan_hover()
                };
                let right_label = if tile_info.is_own_territory {
                    &strings.btn_delete
                } else {
                    &strings.btn_fleft
                };

                let fleet_btn = egui::Button::new(RichText::new(right_label).strong().size(13.0))
                    .fill(right_fill.linear_multiply(0.3))
                    .stroke(egui::Stroke::new(1.2_f32, right_glow))
                    .corner_radius(6);

                if ui.add_sized(egui::vec2(btn_w, btn_h), fleet_btn).clicked() {
                    let troops = Some(state.troops * (state.attack_ratio as f64));
                    cancel_intents.push(sow_core::protocol::GameplayIntent::LaunchFleet {
                        target_tile: tile_info.tile_idx,
                        troops,
                    });
                }

                // 3. Ally Button
                let ally_btn =
                    egui::Button::new(RichText::new(&strings.btn_ally).strong().size(13.0))
                        .fill(palette::button_inactive())
                        .stroke(egui::Stroke::new(1.0_f32, palette::neon_cyan()))
                        .corner_radius(6);
                let _ = ui.add_sized(egui::vec2(btn_w, btn_h), ally_btn);

                // 4. Build / Attack Button
                let left_fill = if tile_info.is_own_territory {
                    palette::neon_gold()
                } else {
                    palette::danger()
                };
                let left_glow = if tile_info.is_own_territory {
                    palette::neon_gold_hover()
                } else {
                    palette::danger_border()
                };
                let left_label = if tile_info.is_own_territory {
                    &strings.btn_build
                } else {
                    &strings.btn_attack
                };

                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(btn_w, btn_h), egui::Sense::click());
                let is_hovered = resp.hovered();
                let fill = if is_hovered {
                    left_fill.linear_multiply(0.4)
                } else {
                    left_fill.linear_multiply(0.3)
                };
                ui.painter().rect(
                    rect,
                    6.0,
                    fill,
                    egui::Stroke::new(1.2_f32, left_glow),
                    egui::StrokeKind::Inside,
                );

                let font_id = egui::FontId::proportional(13.0);
                let galley = ui.painter().layout_no_wrap(
                    left_label.to_owned(),
                    font_id.clone(),
                    Color32::WHITE,
                );
                let start_x = rect.center().x - galley.rect.width() / 2.0;
                ui.painter().galley(
                    pos2(start_x, rect.center().y - galley.rect.height() / 2.0),
                    galley,
                    Color32::WHITE,
                );

                if resp.clicked() && !tile_info.is_own_territory {
                    let troops = state.troops * (state.attack_ratio as f64);
                    if troops > 0.0 {
                        let attack = sow_core::protocol::AttackIntent {
                            target_owner: tile_info.owner_id,
                            troops: Some(troops),
                        };
                        cancel_intents.push(sow_core::protocol::GameplayIntent::Attack(attack));
                    }
                }
            });
            ui.add_space(4.0);
        });
    }
}
fn draw_nuke_alerts(ctx: &Context, state: &mut HudState) {
    const LIFETIME: Duration = Duration::from_millis(5000);
    const FADE_START: f32 = 0.8;
    let now = Instant::now();

    state
        .nuke_alerts
        .retain(|a| now.duration_since(a.spawned_at) < LIFETIME);

    if state.nuke_alerts.is_empty() {
        return;
    }

    let screen_rect = ctx.content_rect();
    let compact = screen_rect.width() < 1024.0 || screen_rect.width() < screen_rect.height() * 1.25;

    let max_visible = if compact { 2 } else { 8 };
    let start = state.nuke_alerts.len().saturating_sub(max_visible);
    let visible = &state.nuke_alerts[start..];

    let area_id = if compact {
        egui::Id::new("nuke_alerts_compact_area_v2")
    } else {
        egui::Id::new("nuke_alerts_desktop_area_v3")
    };

    let anchor = if compact {
        egui::Align2::CENTER_BOTTOM
    } else {
        egui::Align2::LEFT_CENTER
    };

    let offset = if compact {
        egui::vec2(0.0, -185.0 - state.safe_area_bottom)
    } else {
        egui::vec2(16.0, 0.0)
    };

    egui::Area::new(area_id)
        .anchor(anchor, offset)
        .order(egui::Order::Tooltip)
        .movable(false)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                let screen_w = ui.ctx().content_rect().width();
                let max_w = Some(if compact {
                    screen_w * 0.24
                } else {
                    screen_w * 0.33
                });

                for alert in visible {
                    let elapsed = now.duration_since(alert.spawned_at).as_secs_f32();
                    let remaining = LIFETIME.as_secs_f32() - elapsed;
                    let alpha = if remaining < FADE_START {
                        (remaining / FADE_START).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };

                    let bg = Color32::from_rgba_unmultiplied(15, 10, 5, (200.0 * alpha) as u8);
                    let border = alert.color.linear_multiply(alpha);

                    if !compact {
                        let entry_t = (elapsed / 0.25).clamp(0.0, 1.0);
                        let slide = (1.0 - entry_t) * -20.0;
                        ui.add_space(slide);
                    }

                    if let Some(w) = max_w {
                        ui.set_max_width(w);
                    }

                    egui::Frame::new()
                        .fill(bg)
                        .stroke(Stroke::new(1.5_f32, border))
                        .corner_radius(8)
                        .inner_margin(egui::Margin::symmetric(14, 6))
                        .show(ui, |ui| {
                            if let Some(w) = max_w {
                                ui.set_width(w - 28.0);
                            }
                            ui.horizontal_wrapped(|ui| {
                                let is_nuke = alert.message.contains("☢")
                                    || alert.message.to_lowercase().contains("nuke")
                                    || alert.message.to_lowercase().contains("missile");
                                if is_nuke {
                                    let icon_size = if compact { 12.0 } else { 14.0 };
                                    ui.label(
                                        RichText::new("☢").color(border).size(icon_size).strong(),
                                    );
                                    ui.add_space(4.0);
                                }
                                ui.label(
                                    RichText::new(&alert.message)
                                        .color(Color32::from_rgba_unmultiplied(
                                            255,
                                            255,
                                            255,
                                            (255.0 * alpha) as u8,
                                        ))
                                        .size(12.0)
                                        .strong(),
                                );
                            });
                        });
                }
            });
        });

    ctx.request_repaint();
}

fn draw_error_overlay(ctx: &Context, state: &mut HudState) {
    let is_active = state.show_error.is_some();
    let progress =
        ctx.animate_bool_with_time(egui::Id::new("error_toast_animation"), is_active, 0.22);

    if progress <= 0.01 && !is_active {
        state.last_error_message = None;
        return;
    }

    if let Some(err_msg) = state.show_error.clone() {
        let now = Instant::now();
        let display_duration = Duration::from_millis(2500);

        let reset = state.last_error_message.as_ref() != Some(&err_msg);

        if reset {
            state.last_error_message = Some(err_msg.clone());
            state.error_display_timer = Some(now);
        }

        let start_time = state.error_display_timer.unwrap_or(now);
        let elapsed = now.duration_since(start_time);

        if elapsed >= display_duration {
            state.show_error = None;
            state.error_display_timer = None;
        }
    }

    let err_msg = match &state.last_error_message {
        Some(msg) => msg.clone(),
        None => return,
    };

    // Disney overshoot curve (pop-in pop-out spring animation)
    let anim_scale = if is_active {
        let t = progress;
        if t >= 1.0 {
            1.0
        } else {
            1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
        }
    } else {
        progress
    };

    let alpha = progress;
    let bg_color = Color32::from_rgba_unmultiplied(15, 23, 42, (180.0 * alpha) as u8);
    let border_color = crate::ui::theme::accent_danger().linear_multiply(alpha);
    let text_color = Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8);

    let target_y = 80.0 + state.safe_area_top;
    // Slide down from above the screen (-120px) to target with a beautiful overshoot bounce
    let current_y = target_y - 120.0 * (1.0 - anim_scale);

    egui::Area::new(egui::Id::new("error_toast_area"))
        .anchor(egui::Align2::CENTER_TOP, vec2(0.0, current_y))
        .order(egui::Order::Tooltip)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(bg_color)
                .stroke(egui::Stroke::new(1.0_f32, border_color))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(16, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("⚠️").color(border_color).size(12.0));
                        ui.add_space(6.0);
                        ui.label(RichText::new(err_msg).color(text_color).size(12.0).strong());
                    });
                });
        });

    // Request repaint so the fade-out/pop-out animation runs smoothly
    ctx.request_repaint();
}

fn draw_transfer_panel(
    ui: &mut egui::Ui,
    state: &mut HudState,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
) {
    let is_active = state.show_ask_panel.is_some();
    let progress = ui.ctx().animate_bool_with_time(
        egui::Id::new("transfer_panel_animation"),
        is_active,
        0.22,
    );

    if progress <= 0.01 && !is_active {
        return;
    }

    let target_id = if let Some(id) = state.show_ask_panel {
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("transfer_panel_active_target"), id));
        id
    } else {
        ui.ctx().data(|d| d.get_temp::<u16>(egui::Id::new("transfer_panel_active_target"))).unwrap_or(0)
    };

    if target_id == 0 {
        return;
    }

    let target_player = state.players.iter().find(|p| p.id == target_id);
    let target_name = target_player
        .map(|p| {
            if p.name.is_empty() {
                if p.id >= 200 {
                    format!("Tribe {}", p.id - 199)
                } else {
                    format!("Nation {}", p.id - 103)
                }
            } else {
                p.name.clone()
            }
        })
        .unwrap_or_else(|| format!("Ally {}", target_id));

    // Active Tab: 0 = Send, 1 = Request
    let mut active_tab = ui.ctx().data(|d| d.get_temp::<usize>(egui::Id::new("transfer_active_tab"))).unwrap_or(0);

    // Dynamic max bounds based on tab
    let (max_gold, max_troops, balance_label, accent_color) = if active_tab == 0 {
        (state.gold, state.troops, "Your Balance", crate::ui::theme::accent_solo_cyan())
    } else {
        let ally_gold = target_player.map(|p| p.gold).unwrap_or(0.0);
        let ally_troops = target_player.map(|p| p.troops).unwrap_or(0.0);
        (ally_gold, ally_troops, "Ally Balance", crate::ui::theme::accent_ranked_gold())
    };

    // Clamp values if tab switches and current value exceeds new bounds
    if state.ask_gold > max_gold {
        state.ask_gold = max_gold;
    }
    if state.ask_troops > max_troops {
        state.ask_troops = max_troops;
    }

    // Disney overshoot curve (pop-in pop-out spring animation)
    let anim_scale = if is_active {
        let t = progress;
        if t >= 1.0 {
            1.0
        } else {
            1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
        }
    } else {
        progress
    };

    let alpha = progress;

    // Backdrop
    let backdrop_color = Color32::from_black_alpha((100.0 * alpha) as u8);
    let screen_rect = ui.ctx().content_rect();
    ui.ctx()
        .layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("transfer_panel_backdrop"),
        ))
        .rect_filled(screen_rect, 0.0, backdrop_color);

    let target_y = screen_rect.center().y;
    // Slide up with overshoot bounce from below screen
    let current_y = target_y + (screen_rect.height() / 2.0 + 200.0) * (1.0 - anim_scale);

    let compact = screen_rect.width() < 1024.0 || screen_rect.width() < screen_rect.height() * 1.25;
    let modal_w = if compact { 320.0 } else { 380.0 };

    egui::Area::new(egui::Id::new("transfer_panel_modal"))
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, current_y - target_y))
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            ui.set_width(modal_w);

            let frame = crate::ui::theme::standard_panel_frame(false)
                .fill(crate::ui::theme::panel_bg().linear_multiply(alpha));

            let frame_res = frame.show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 10.0);

                    // Title
                    ui.vertical_centered(|ui| {
                        crate::ui::theme::outlined_label(
                            ui,
                            "🤝 Resource Transfer",
                            egui::FontId::proportional(20.0),
                            Color32::WHITE,
                        );
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(format!("with {}", target_name))
                                .size(14.0)
                                .color(crate::ui::theme::text_secondary().linear_multiply(alpha))
                                .strong(),
                        );
                    });

                    ui.add_space(6.0);

                    // --- DEFI TABS ---
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let tab_w = (ui.available_width() - 8.0) / 2.0;

                        // Send Tab Button
                        let is_send = active_tab == 0;
                        let send_btn = egui::Button::new(
                            RichText::new("📤 SEND")
                                .strong()
                                .size(14.0)
                                .color(if is_send { Color32::WHITE } else { Color32::GRAY }),
                        )
                        .fill(if is_send {
                            crate::ui::theme::accent_solo_cyan().linear_multiply(0.4)
                        } else {
                            crate::ui::theme::menu_secondary_button()
                        })
                        .stroke(egui::Stroke::new(
                            1.5_f32,
                            if is_send { crate::ui::theme::accent_solo_cyan() } else { Color32::TRANSPARENT },
                        ))
                        .corner_radius(8);

                        if ui.add_sized(vec2(tab_w, 32.0), send_btn).clicked() {
                            active_tab = 0;
                            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("transfer_active_tab"), 0_usize));
                        }

                        // Request Tab Button
                        let is_req = active_tab == 1;
                        let req_btn = egui::Button::new(
                            RichText::new("📥 REQUEST")
                                .strong()
                                .size(14.0)
                                .color(if is_req { Color32::WHITE } else { Color32::GRAY }),
                        )
                        .fill(if is_req {
                            crate::ui::theme::accent_ranked_gold().linear_multiply(0.4)
                        } else {
                            crate::ui::theme::menu_secondary_button()
                        })
                        .stroke(egui::Stroke::new(
                            1.5_f32,
                            if is_req { crate::ui::theme::accent_ranked_gold() } else { Color32::TRANSPARENT },
                        ))
                        .corner_radius(8);

                        if ui.add_sized(vec2(tab_w, 32.0), req_btn).clicked() {
                            active_tab = 1;
                            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("transfer_active_tab"), 1_usize));
                        }
                    });

                    ui.add_space(4.0);

                    // --- GOLD SECTION ---
                    ui.group(|ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("💰 Gold").strong().size(15.0));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(format!("{}", crate::utils::format_number(state.ask_gold)))
                                            .color(crate::ui::theme::accent_ranked_gold())
                                            .strong()
                                            .size(15.0),
                                    );
                                });
                            });

                            ui.add_space(2.0);

                            // Balance label
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(balance_label).size(11.0).color(crate::ui::theme::text_secondary()));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(crate::utils::format_number(max_gold))
                                            .size(11.0)
                                            .color(Color32::LIGHT_GRAY)
                                            .strong(),
                                    );
                                });
                            });

                            ui.add_space(4.0);

                            // Gold slider
                            let slider_width = ui.available_width();
                            ui.add_sized(
                                egui::vec2(slider_width, ui.spacing().interact_size.y),
                                Slider::new(&mut state.ask_gold, 0.0..=max_gold.max(1.0))
                                    .show_value(false)
                                    .integer(),
                            );

                            ui.add_space(4.0);

                            // Presets Row
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                let percentages = [0.25, 0.50, 0.75, 1.0];
                                for &pct in &percentages {
                                    let val = (max_gold * pct).floor();
                                    let btn_label = format!("{:.0}%", pct * 100.0);
                                    if ui.button(RichText::new(btn_label).size(12.0)).clicked() {
                                        state.ask_gold = val;
                                    }
                                }
                            });
                        });
                    });

                    ui.add_space(4.0);

                    // --- TROOPS SECTION ---
                    ui.group(|ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("🛡️ Troops").strong().size(15.0));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(format!("{}", crate::utils::format_number(state.ask_troops)))
                                            .color(crate::ui::theme::accent_solo_cyan())
                                            .strong()
                                            .size(15.0),
                                    );
                                });
                            });

                            ui.add_space(2.0);

                            // Balance label
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(balance_label).size(11.0).color(crate::ui::theme::text_secondary()));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(crate::utils::format_number(max_troops))
                                            .size(11.0)
                                            .color(Color32::LIGHT_GRAY)
                                            .strong(),
                                    );
                                });
                            });

                            ui.add_space(4.0);

                            // Troops slider
                            let slider_width = ui.available_width();
                            ui.add_sized(
                                egui::vec2(slider_width, ui.spacing().interact_size.y),
                                Slider::new(&mut state.ask_troops, 0.0..=max_troops.max(1.0))
                                    .show_value(false)
                                    .integer(),
                            );

                            ui.add_space(4.0);

                            // Presets Row
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                let percentages = [0.25, 0.50, 0.75, 1.0];
                                for &pct in &percentages {
                                    let val = (max_troops * pct).floor();
                                    let btn_label = format!("{:.0}%", pct * 100.0);
                                    if ui.button(RichText::new(btn_label).size(12.0)).clicked() {
                                        state.ask_troops = val;
                                    }
                                }
                            });
                        });
                    });

                    ui.add_space(10.0);

                    // --- ACTION BUTTONS ---
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;

                        // Cancel Button
                        let cancel_btn = egui::Button::new(
                            RichText::new("CANCEL")
                                .strong()
                                .size(14.0)
                                .color(Color32::LIGHT_GRAY),
                        )
                        .fill(crate::ui::theme::menu_secondary_button())
                        .corner_radius(8);

                        let btn_w = (ui.available_width() - 10.0) / 2.0;
                        if ui.add_sized(vec2(btn_w, 36.0), cancel_btn).clicked() {
                            state.show_ask_panel = None;
                        }

                        // Submit Button
                        let is_valid = state.ask_gold > 0.0 || state.ask_troops > 0.0;
                        let btn_text = if active_tab == 0 { "SEND" } else { "REQUEST" };

                        let submit_btn = egui::Button::new(
                            RichText::new(btn_text)
                                .strong()
                                .size(14.0)
                                .color(if is_valid { Color32::WHITE } else { Color32::GRAY }),
                        )
                        .fill(if is_valid {
                            accent_color
                        } else {
                            crate::ui::theme::menu_secondary_button()
                        })
                        .corner_radius(8);

                        let submit_resp = ui.add_sized(vec2(btn_w, 36.0), submit_btn);
                        if is_valid && submit_resp.clicked() {
                            if active_tab == 0 {
                                cancel_intents.push(sow_core::protocol::GameplayIntent::SendResources {
                                    target_player: target_id,
                                    gold: state.ask_gold,
                                    troops: state.ask_troops,
                                });
                            } else {
                                cancel_intents.push(sow_core::protocol::GameplayIntent::RequestResources {
                                    target_player: target_id,
                                    gold: state.ask_gold,
                                    troops: state.ask_troops,
                                });
                            }

                            // Reset values and close panel
                            state.ask_gold = 0.0;
                            state.ask_troops = 0.0;
                            state.show_ask_panel = None;
                        }
                    });
                });
            });

            let response_rect = frame_res.response.rect;
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("transfer_panel_rect"), response_rect));
        });

    // Click outside the ask panel closes it
    if ui.ctx().input(|i| i.pointer.any_pressed()) {
        if let Some(pos) = ui
            .ctx()
            .input(|i| i.pointer.press_origin().or(i.pointer.interact_pos()))
        {
            let mut click_absorbed = false;
            if let Some(rect) = ui
                .ctx()
                .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("transfer_panel_rect")))
            {
                if rect.contains(pos) {
                    click_absorbed = true;
                }
            }
            if !click_absorbed && is_active {
                state.show_ask_panel = None;
            }
        }
    }

    ui.ctx().request_repaint();
}
