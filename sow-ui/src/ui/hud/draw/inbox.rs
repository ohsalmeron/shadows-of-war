use egui::{Align2, Color32, vec2};
use sow_i18n::Language;

use super::super::state::HudState;
use crate::ui::asset_loader::AssetLoader;

pub(in crate::ui::hud) fn draw_alliance_inbox(
    ui: &mut egui::Ui,
    state: &mut HudState,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    lang: Language,
    anim: f32,
    _asset_loader: &mut AssetLoader,
) {
    let my_snapshot = state.players.iter().find(|p| p.id == state.my_player_id);
    let requests = my_snapshot
        .map(|p| p.alliance_requests.clone())
        .unwrap_or_default();
    let resource_requests = my_snapshot
        .map(|p| p.resource_requests.clone())
        .unwrap_or_default();
    let total_notifications = requests.len() + resource_requests.len();

    // ── Floating Alliance Inbox Panel ─────────────────────────────────────────
    let is_inbox_active = state.show_alliance_inbox;
    let inbox_progress = ui.ctx().animate_bool_with_time(
        egui::Id::new("alliance_inbox_animation"),
        is_inbox_active,
        anim,
    );
    if inbox_progress > 0.01 {
        let anim_scale = if is_inbox_active {
            let t = inbox_progress;
            if t >= 1.0 {
                1.0
            } else {
                crate::ui::animation::spring_overshoot(t)
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
                    let prepaint_idx = ui.painter().add(egui::Shape::Noop);
                    let frame_res = egui::Frame::NONE
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
                                ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);

                                // Title "ALLIANCES"
                                sow_ui_kit::theme::outlined_label(
                                    ui,
                                    &sow_i18n::get(lang).hud.inbox_title,
                                    egui::FontId::proportional(12.0),
                                    sow_ui_kit::theme::palette::neon_cyan().linear_multiply(inbox_progress),
                                );
                                ui.add_space(4.0);

                                // Reject All / Accept All — only when 2+ requests
                                if requests.len() > 1 {
                                    let w = (ui.available_width() - 6.0) / 2.0;
                                    ui.horizontal(|ui| {
                                        if ui.add(
                                            crate::widgets::ThemeButton::new(&sow_i18n::get(lang).hud.reject_all)
                                                .min_size(egui::vec2(w, 24.0))
                                                .text_size(10.0)
                                                .custom_fill(sow_ui_kit::theme::palette::button_inactive())
                                                .custom_text_color(Color32::from_rgb(239, 68, 68).linear_multiply(inbox_progress))
                                        ).clicked() {
                                            for &req in &requests {
                                                cancel_intents.push(sow_core::protocol::GameplayIntent::RejectAlliance { target_player: req });
                                            }
                                            state.show_alliance_inbox = false;
                                        }
                                        if ui.add(
                                            crate::widgets::ThemeButton::new(&sow_i18n::get(lang).hud.accept_all)
                                                .min_size(egui::vec2(w, 24.0))
                                                .text_size(10.0)
                                                .custom_fill(sow_ui_kit::theme::palette::button_inactive())
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
                                    sow_ui_kit::theme::outlined_label(
                                        ui,
                                        &sow_i18n::get(lang).hud.inbox_empty,
                                        egui::FontId::proportional(11.0),
                                        Color32::GRAY.linear_multiply(inbox_progress),
                                    );
                                }
                                for &requester_id in &requests {
                                    let Some(requester) = state.players.iter().find(|p| p.id == requester_id) else { continue };
                                    let is_renewal = my_snapshot
                                        .map(|me| me.alliances.contains(&requester_id))
                                        .unwrap_or(false);
                                    let rgb = if let Some(team) = requester.team {
                                        sow_core::player::team_territory_rgb(team)
                                    } else if requester.player_type == sow_core::player::PlayerType::Human {
                                        sow_core::player::human_shader_territory_rgb(requester.id)
                                    } else {
                                        requester.color
                                    };
                                    let pc = Color32::from_rgb(
                                        (rgb[0] * 255.0) as u8,
                                        (rgb[1] * 255.0) as u8,
                                        (rgb[2] * 255.0) as u8,
                                    ).linear_multiply(inbox_progress);
                                    let icon = if requester.disconnected { "🔌" } else if requester.id < 200 { "⭐" } else { sow_core::player::tribe_animal(requester.id, &requester.name) };
                                    let name = if requester.player_type == sow_core::player::PlayerType::Bot {
                                        if requester.name.is_empty() { format!("Tribe {}", requester.id.saturating_sub(199)) } else { requester.name.clone() }
                                    } else {
                                        sow_core::player::display_name(requester.id, &requester.name, requester.player_type)
                                    };

                                    // Animate individual card sliding horizontally with spring overshoot!
                                    let card_progress = ui.ctx().animate_bool_with_time(egui::Id::new(("request_card", requester_id)), true, anim);
                                    let card_scale = crate::ui::animation::spring_overshoot(card_progress);
                                    let card_offset = 30.0 * (1.0 - card_scale.max(0.0));

                                    ui.horizontal(|ui| {
                                        if card_offset > 0.01 {
                                            ui.add_space(card_offset);
                                        }

                                        egui::Frame::NONE
                                            .fill(sow_ui_kit::theme::palette::field_bg().linear_multiply(0.5 * inbox_progress * card_progress))
                                            .stroke(egui::Stroke::new(1.0_f32, sow_ui_kit::theme::palette::field_border().linear_multiply(0.4 * inbox_progress * card_progress)))
                                            .corner_radius(6)
                                            .inner_margin(egui::Margin::symmetric(8, 6))
                                            .show(ui, |ui| {
                                                ui.vertical(|ui| {
                                                    // Name row
                                                    ui.horizontal(|ui| {
                                                        let icon_size = 24.0;
                                                        let (icon_rect, _) = ui.allocate_exact_size(
                                                            egui::vec2(icon_size, icon_size),
                                                            egui::Sense::hover(),
                                                        );
                                                        if !crate::widgets::try_paint_emoji(
                                                            ui.painter(),
                                                            icon,
                                                            icon_rect,
                                                            pc,
                                                        ) {
                                                            crate::widgets::outlined_emoji_label(
                                                                ui,
                                                                icon,
                                                                egui::FontId::proportional(18.0),
                                                                pc,
                                                            );
                                                        }
                                                        ui.vertical(|ui| {
                                                            ui.spacing_mut().item_spacing.y = 0.0;
                                                            crate::widgets::outlined_emoji_label(ui, &name, egui::FontId::proportional(12.5), pc);
                                                            let prompt = if is_renewal {
                                                                match lang {
                                                                    sow_i18n::Language::Spanish => "¡quiere renovar la alianza!".to_string(),
                                                                    _ => "wants to renew your alliance!".to_string(),
                                                                }
                                                            } else {
                                                                sow_i18n::get(lang).hud.inbox_wants_ally.clone()
                                                            };
                                                            crate::widgets::outlined_emoji_label(ui, &prompt, egui::FontId::proportional(10.5), Color32::LIGHT_GRAY.linear_multiply(inbox_progress * card_progress));
                                                        });
                                                    });
                                                    ui.add_space(2.0);
                                                    // Button row
                                                    let bw = (ui.available_width() - 6.0) / 2.0;
                                                    let is_last = total_notifications == 1;
                                                    ui.horizontal(|ui| {
                                                        if ui.add(
                                                            crate::widgets::ThemeButton::new(&sow_i18n::get(lang).hud.btn_accept)
                                                                .min_size(egui::vec2(bw, 24.0))
                                                                .text_size(11.0)
                                                                .custom_fill(sow_ui_kit::theme::palette::button_inactive())
                                                                .custom_text_color(Color32::from_rgb(74, 222, 128).linear_multiply(inbox_progress))
                                                        ).clicked() {
                                                            cancel_intents.push(sow_core::protocol::GameplayIntent::AcceptAlliance { target_player: requester.id });
                                                            if is_last { state.show_alliance_inbox = false; }
                                                        }
                                                        if ui.add(
                                                            crate::widgets::ThemeButton::new(&sow_i18n::get(lang).hud.btn_reject)
                                                                .min_size(egui::vec2(bw, 24.0))
                                                                .text_size(11.0)
                                                                .custom_fill(sow_ui_kit::theme::palette::button_inactive())
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
                                    let rgb = if let Some(team) = requester.team {
                                        sow_core::player::team_territory_rgb(team)
                                    } else if requester.player_type == sow_core::player::PlayerType::Human {
                                        sow_core::player::human_shader_territory_rgb(requester.id)
                                    } else {
                                        requester.color
                                    };
                                    let pc = Color32::from_rgb(
                                        (rgb[0] * 255.0) as u8,
                                        (rgb[1] * 255.0) as u8,
                                        (rgb[2] * 255.0) as u8,
                                    ).linear_multiply(inbox_progress);
                                    let icon = if requester.disconnected { "🔌" } else if requester.id < 200 { "⭐" } else { sow_core::player::tribe_animal(requester.id, &requester.name) };
                                    let name = if requester.player_type == sow_core::player::PlayerType::Bot {
                                        if requester.name.is_empty() { format!("Tribe {}", requester.id.saturating_sub(199)) } else { requester.name.clone() }
                                    } else {
                                        sow_core::player::display_name(requester.id, &requester.name, requester.player_type)
                                    };

                                    let card_progress = ui.ctx().animate_bool_with_time(egui::Id::new(("res_request_card", requester_id)), true, anim);
                                    let card_scale = crate::ui::animation::spring_overshoot(card_progress);
                                    let card_offset = 30.0 * (1.0 - card_scale.max(0.0));

                                    ui.horizontal(|ui| {
                                        if card_offset > 0.01 {
                                            ui.add_space(card_offset);
                                        }

                                        egui::Frame::NONE
                                            .fill(sow_ui_kit::theme::palette::field_bg().linear_multiply(0.5 * inbox_progress * card_progress))
                                            .stroke(egui::Stroke::new(1.0_f32, sow_ui_kit::theme::palette::field_border().linear_multiply(0.4 * inbox_progress * card_progress)))
                                            .corner_radius(6)
                                            .inner_margin(egui::Margin::symmetric(8, 6))
                                            .show(ui, |ui| {
                                                ui.vertical(|ui| {
                                                    // Name row
                                                    ui.horizontal(|ui| {
                                                        let icon_size = 24.0;
                                                        let (icon_rect, _) = ui.allocate_exact_size(
                                                            egui::vec2(icon_size, icon_size),
                                                            egui::Sense::hover(),
                                                        );
                                                        if !crate::widgets::try_paint_emoji(
                                                            ui.painter(),
                                                            icon,
                                                            icon_rect,
                                                            pc,
                                                        ) {
                                                            crate::widgets::outlined_emoji_label(
                                                                ui,
                                                                icon,
                                                                egui::FontId::proportional(18.0),
                                                                pc,
                                                            );
                                                        }
                                                        ui.vertical(|ui| {
                                                            ui.spacing_mut().item_spacing.y = 0.0;
                                                            crate::widgets::outlined_emoji_label(ui, &name, egui::FontId::proportional(12.5), pc);
                                                            let prompt = match (req.gold > 0.0, req.troops > 0.0) {
                                                                (true, true) => format!("asks for 🪙{} & 🛡️{}", crate::utils::format_number(req.gold), crate::utils::format_number(req.troops)),
                                                                (true, false) => format!("asks for 🪙{}", crate::utils::format_number(req.gold)),
                                                                (false, true) => format!("asks for 🛡️{}", crate::utils::format_number(req.troops)),
                                                                _ => "asks for resources".to_string(),
                                                            };
                                                            crate::widgets::outlined_emoji_label(
                                                                ui,
                                                                &prompt,
                                                                egui::FontId::proportional(10.5),
                                                                Color32::LIGHT_GRAY.linear_multiply(inbox_progress * card_progress),
                                                            );
                                                        });
                                                    });
                                                    ui.add_space(2.0);
                                                    // Button row
                                                    let bw = (ui.available_width() - 6.0) / 2.0;
                                                    let is_last = total_notifications == 1;
                                                    ui.horizontal(|ui| {
                                                        if ui.add(
                                                            crate::widgets::ThemeButton::new(&sow_i18n::get(lang).hud.btn_accept)
                                                                .min_size(egui::vec2(bw, 24.0))
                                                                .text_size(11.0)
                                                                .custom_fill(sow_ui_kit::theme::palette::button_inactive())
                                                                .custom_text_color(Color32::from_rgb(74, 222, 128).linear_multiply(inbox_progress))
                                                        ).clicked() {
                                                            cancel_intents.push(sow_core::protocol::GameplayIntent::AcceptResourceRequest { target_player: requester.id });
                                                            if is_last { state.show_alliance_inbox = false; }
                                                        }
                                                        if ui.add(
                                                            crate::widgets::ThemeButton::new(&sow_i18n::get(lang).hud.btn_reject)
                                                                .min_size(egui::vec2(bw, 24.0))
                                                                .text_size(11.0)
                                                                .custom_fill(sow_ui_kit::theme::palette::button_inactive())
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
                    let compact = sow_ui_kit::theme::compact_viewport(ui.ctx());
                    sow_ui_kit::theme::paint_hud_panel_gradient(
                        ui,
                        prepaint_idx,
                        frame_res.response.rect,
                        sow_ui_kit::theme::palette::neon_cyan().linear_multiply(inbox_progress),
                        if compact { egui::CornerRadius::ZERO } else { sow_ui_kit::theme::radius::lg() },
                    );
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

                if let Some(rect) = ui
                    .ctx()
                    .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("hud_top_icons_rect")))
                {
                    if rect.contains(pos) {
                        click_absorbed = true;
                    }
                }

                if !click_absorbed {
                    state.show_alliance_inbox = false;
                }
            }
        }
        ui.ctx().request_repaint();
    }
}
