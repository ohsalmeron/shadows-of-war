use egui::{vec2, Color32, RichText, Slider};
use sow_i18n::Language;

use super::super::state::HudState;

pub(in crate::ui::hud) fn transfer_needs_confirm(
    state: &HudState,
    max_gold: f64,
    max_troops: f64,
) -> bool {
    let gold_pct = if max_gold > 0.0 {
        state.ask_gold / max_gold
    } else {
        0.0
    };
    let troop_pct = if max_troops > 0.0 {
        state.ask_troops / max_troops
    } else {
        0.0
    };
    gold_pct > 0.5 || troop_pct > 0.5
}

pub(in crate::ui::hud) fn draw_transfer_panel(
    ui: &mut egui::Ui,
    state: &mut HudState,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    lang: Language,
) {
    let strings = &sow_i18n::get(lang).hud;
    let is_active = state.show_ask_panel.is_some();
    let anim = sow_ui_kit::theme::anim_duration_from_ctx(ui.ctx());
    let progress =
        ui.ctx()
            .animate_bool_with_time(egui::Id::new("transfer_panel_animation"), is_active, anim);

    if progress <= 0.01 && !is_active {
        return;
    }

    let target_id = if let Some(id) = state.show_ask_panel {
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("transfer_panel_active_target"), id));
        id
    } else {
        ui.ctx()
            .data(|d| d.get_temp::<u16>(egui::Id::new("transfer_panel_active_target")))
            .unwrap_or(0)
    };

    if target_id == 0 {
        return;
    }

    let target_player = state.players.iter().find(|p| p.id == target_id);
    let target_name = target_player
        .map(|p| sow_core::player::display_name(p.id, &p.name, p.player_type))
        .unwrap_or_else(|| format!("Ally {}", target_id));

    // Active Tab: 0 = Send, 1 = Request
    let mut active_tab = ui
        .ctx()
        .data(|d| d.get_temp::<usize>(egui::Id::new("transfer_active_tab")))
        .unwrap_or(0);

    // Dynamic max bounds based on tab
    let (max_gold, max_troops, balance_label, accent_color) = if active_tab == 0 {
        (
            state.gold,
            state.troops,
            "Your Balance",
            sow_ui_kit::theme::palette::neon_cyan(),
        )
    } else {
        let ally_gold = target_player.map(|p| p.gold).unwrap_or(0.0);
        let ally_troops = target_player.map(|p| p.troops).unwrap_or(0.0);
        (
            ally_gold,
            ally_troops,
            "Ally Balance",
            sow_ui_kit::theme::palette::neon_gold(),
        )
    };

    // Clamp values if tab switches and current value exceeds new bounds
    if state.ask_gold > max_gold {
        state.ask_gold = max_gold;
    }
    if state.ask_troops > max_troops {
        state.ask_troops = max_troops;
    }

    let anim_scale = if is_active {
        let t = progress;
        if t >= 1.0 {
            1.0
        } else {
            crate::ui::animation::spring_overshoot(t)
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

    let compact = screen_rect.width() < 768.0 || screen_rect.width() < screen_rect.height() * 1.25;
    let modal_w = if compact { 320.0 } else { 380.0 };

    egui::Area::new(egui::Id::new("transfer_panel_modal"))
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, current_y - target_y))
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            ui.set_width(modal_w);

            let frame = sow_ui_kit::theme::standard_panel_frame(false)
                .fill(sow_ui_kit::theme::palette::surface().linear_multiply(alpha));

            let frame_res = frame.show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 10.0);

                    // Title
                    ui.vertical_centered(|ui| {
                        sow_ui_kit::theme::outlined_label(
                            ui,
                            &strings.transfer_title,
                            egui::FontId::proportional(20.0),
                            Color32::WHITE,
                        );
                        ui.add_space(2.0);
                        let with_text = format!("with {}", target_name);
                        crate::widgets::emoji_label(
                            ui,
                            &with_text,
                            egui::FontId::proportional(14.0),
                            sow_ui_kit::theme::palette::text_muted().linear_multiply(alpha),
                        );
                    });

                    ui.add_space(6.0);

                    // --- DEFI TABS ---
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let tab_w = (ui.available_width() - 8.0) / 2.0;

                        // Send Tab Button
                        let is_send = active_tab == 0;
                        let send_btn = crate::widgets::ThemeButton::new(&strings.transfer_send)
                            .style(if is_send {
                                crate::widgets::ThemeButtonStyle::Primary
                            } else {
                                crate::widgets::ThemeButtonStyle::Tertiary
                            })
                            .custom_fill(if is_send {
                                sow_ui_kit::theme::palette::neon_cyan().linear_multiply(0.4)
                            } else {
                                sow_ui_kit::theme::palette::button_inactive()
                            })
                            .stroke(egui::Stroke::new(
                                1.5_f32,
                                if is_send {
                                    sow_ui_kit::theme::palette::neon_cyan()
                                } else {
                                    Color32::TRANSPARENT
                                },
                            ))
                            .min_size(vec2(tab_w, 32.0))
                            .text_size(14.0);

                        if ui.add(send_btn).clicked() {
                            active_tab = 0;
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(egui::Id::new("transfer_active_tab"), 0_usize)
                            });
                        }

                        // Request Tab Button
                        let is_req = active_tab == 1;
                        let req_btn = crate::widgets::ThemeButton::new(&strings.transfer_request)
                            .style(if is_req {
                                crate::widgets::ThemeButtonStyle::Secondary
                            } else {
                                crate::widgets::ThemeButtonStyle::Tertiary
                            })
                            .custom_fill(if is_req {
                                sow_ui_kit::theme::palette::neon_gold().linear_multiply(0.4)
                            } else {
                                sow_ui_kit::theme::palette::button_inactive()
                            })
                            .stroke(egui::Stroke::new(
                                1.5_f32,
                                if is_req {
                                    sow_ui_kit::theme::palette::neon_gold()
                                } else {
                                    Color32::TRANSPARENT
                                },
                            ))
                            .min_size(vec2(tab_w, 32.0))
                            .text_size(14.0);

                        if ui.add(req_btn).clicked() {
                            active_tab = 1;
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(egui::Id::new("transfer_active_tab"), 1_usize)
                            });
                        }
                    });

                    ui.add_space(4.0);

                    // --- GOLD SECTION ---
                    ui.group(|ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                crate::widgets::emoji_label(
                                    ui,
                                    "🪙 Gold",
                                    egui::FontId::proportional(15.0),
                                    Color32::WHITE,
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(crate::utils::format_number(
                                                state.ask_gold,
                                            ))
                                            .color(sow_ui_kit::theme::palette::neon_gold())
                                            .strong()
                                            .size(15.0),
                                        );
                                    },
                                );
                            });

                            ui.add_space(2.0);

                            // Balance label
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(balance_label)
                                        .size(11.0)
                                        .color(sow_ui_kit::theme::palette::text_muted()),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(crate::utils::format_number(max_gold))
                                                .size(11.0)
                                                .color(Color32::LIGHT_GRAY)
                                                .strong(),
                                        );
                                    },
                                );
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
                                crate::widgets::emoji_label(
                                    ui,
                                    "🛡️ Troops",
                                    egui::FontId::proportional(15.0),
                                    Color32::WHITE,
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(crate::utils::format_number(
                                                state.ask_troops,
                                            ))
                                            .color(sow_ui_kit::theme::palette::neon_cyan())
                                            .strong()
                                            .size(15.0),
                                        );
                                    },
                                );
                            });

                            ui.add_space(2.0);

                            // Balance label
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(balance_label)
                                        .size(11.0)
                                        .color(sow_ui_kit::theme::palette::text_muted()),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(crate::utils::format_number(max_troops))
                                                .size(11.0)
                                                .color(Color32::LIGHT_GRAY)
                                                .strong(),
                                        );
                                    },
                                );
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

                    if state.transfer_confirm_pending {
                        ui.label(
                            RichText::new(&strings.transfer_confirm_body)
                                .size(12.0)
                                .color(sow_ui_kit::theme::palette::danger()),
                        );
                        ui.add_space(8.0);
                    }

                    // --- ACTION BUTTONS ---
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;

                        let btn_w = (ui.available_width() - 10.0) / 2.0;

                        let cancel_btn = crate::widgets::ThemeButton::new(&strings.transfer_cancel)
                            .style(crate::widgets::ThemeButtonStyle::Tertiary)
                            .custom_fill(sow_ui_kit::theme::palette::button_inactive())
                            .min_size(vec2(btn_w, 36.0))
                            .text_size(14.0);

                        if ui.add(cancel_btn).clicked() {
                            state.transfer_confirm_pending = false;
                            state.show_ask_panel = None;
                        }

                        let is_valid = state.ask_gold > 0.0 || state.ask_troops > 0.0;
                        let btn_text = if state.transfer_confirm_pending {
                            &strings.transfer_confirm_yes
                        } else if active_tab == 0 {
                            &strings.transfer_send
                        } else {
                            &strings.transfer_request
                        };

                        let submit_btn = crate::widgets::ThemeButton::new(btn_text)
                            .style(if is_valid {
                                crate::widgets::ThemeButtonStyle::Primary
                            } else {
                                crate::widgets::ThemeButtonStyle::Tertiary
                            })
                            .custom_fill(if is_valid {
                                accent_color
                            } else {
                                sow_ui_kit::theme::palette::button_inactive()
                            })
                            .min_size(vec2(btn_w, 36.0))
                            .text_size(14.0);

                        let submit_resp = ui.add(submit_btn);
                        if is_valid && submit_resp.clicked() {
                            if transfer_needs_confirm(state, max_gold, max_troops)
                                && !state.transfer_confirm_pending
                            {
                                state.transfer_confirm_pending = true;
                            } else {
                                if active_tab == 0 {
                                    cancel_intents.push(
                                        sow_core::protocol::GameplayIntent::SendResources {
                                            target_player: target_id,
                                            gold: state.ask_gold,
                                            troops: state.ask_troops,
                                        },
                                    );
                                } else {
                                    cancel_intents.push(
                                        sow_core::protocol::GameplayIntent::RequestResources {
                                            target_player: target_id,
                                            gold: state.ask_gold,
                                            troops: state.ask_troops,
                                        },
                                    );
                                }

                                state.ask_gold = 0.0;
                                state.ask_troops = 0.0;
                                state.transfer_confirm_pending = false;
                                state.show_ask_panel = None;
                            }
                        }
                    });
                });
            });

            let response_rect = frame_res.response.rect;
            ui.ctx()
                .data_mut(|d| d.insert_temp(egui::Id::new("transfer_panel_rect"), response_rect));
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
                state.transfer_confirm_pending = false;
                state.show_ask_panel = None;
            }
        }
    }

    ui.ctx().request_repaint();
}
