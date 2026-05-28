use super::MainMenuState;
use crate::UiAction;
use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, Ui};

pub fn draw_queue_overlay(
    ui: &mut Ui,
    state: &MainMenuState,
    section_gap: f32,
    action_min_h: f32,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_lang::Language,
) {
    let strings = &sow_lang::get(lang).main_menu;
    let compact = super::lobby_compact_layout(ui.ctx());

    // Get lobby information
    let mut lobby_info = None;
    if let Some(lobby_id) = state.joined_lobby_id.or(state.pending_join_lobby_id) {
        if let Some(lobby) = state.lobbies.iter().find(|l| l.id == lobby_id) {
            lobby_info = Some(lobby);
        }
    }

    // 1. Premium standard panel matching main menu
    let panel_frame = crate::ui::theme::standard_panel_frame(compact);
    let parent_available = ui.available_size();
    let pad = if compact { 32.0 } else { 50.0 };
    let inner_size = parent_available - egui::vec2(pad, pad);

    panel_frame.show(ui, |ui| {
        if compact {
            ui.set_min_height(inner_size.y);
        } else {
            ui.set_min_size(inner_size);
        }
        ui.vertical(|ui| {
            if let Some(lobby) = lobby_info {
                // Header (Status / Title / Timer)
                ui.vertical_centered(|ui| {
                    crate::ui::theme::outlined_label(
                        ui,
                        &strings.matchmaking_established,
                        egui::FontId::proportional(if compact { 20.0 } else { 28.0 }),
                        Color32::WHITE,
                    );

                    let timer_text = if lobby.is_counting_down {
                        format!("STARTING IN: {:.1}S", lobby.timer_secs)
                    } else if state.wait_timer_secs > 0.0 {
                        format!("STARTING IN: {:.1}S", state.wait_timer_secs)
                    } else {
                        strings.awaiting_combat_criteria.to_string()
                    };

                    let timer_color = if lobby.is_counting_down || state.wait_timer_secs > 0.0 {
                        Color32::from_rgb(255, 210, 120)
                    } else {
                        crate::ui::theme::text_secondary()
                    };

                    ui.add_space(2.0);
                    crate::ui::theme::outlined_label(
                        ui,
                        &timer_text,
                        egui::FontId::proportional(if compact { 14.0 } else { 18.0 }),
                        timer_color,
                    );
                });

                ui.add_space(section_gap);

                // 2. Middle Flex Content Area
                let button_h = action_min_h + 16.0;
                let middle_h = ui.available_height() - button_h;

                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), middle_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        if compact {
                            ui.vertical(|ui| {
                                // Draw map briefing (fixed height)
                                draw_map_briefing(ui, lobby, asset_loader, true, lang);
                                ui.add_space(8.0);
                                // Draw ready room player list (takes remaining height)
                                let ready_room_h = ui.available_height();
                                ui.allocate_ui_with_layout(
                                    egui::vec2(ui.available_width(), ready_room_h),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        draw_ready_room(ui, lobby, asset_loader, lang);
                                    },
                                );
                            });
                        } else {
                            ui.horizontal_top(|ui| {
                                let total_w = ui.available_width();
                                let col_w = (total_w - 20.0) * 0.5_f32;
                                let col_h = ui.available_height();

                                ui.allocate_ui_with_layout(
                                    egui::vec2(col_w, col_h),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        draw_map_briefing(ui, lobby, asset_loader, false, lang);
                                    },
                                );

                                ui.add_space(20.0);

                                ui.allocate_ui_with_layout(
                                    egui::vec2(col_w, col_h),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        draw_ready_room(ui, lobby, asset_loader, lang);
                                    },
                                );
                            });
                        }
                    },
                );
            } else {
                // Connecting/Syncing state
                let button_h = action_min_h + 16.0;
                let middle_h = ui.available_height() - button_h;
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), middle_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(middle_h * 0.35);
                            ui.add(egui::Spinner::new().size(36.0));
                            ui.add_space(16.0);
                            crate::ui::theme::outlined_label(
                                ui,
                                &strings.establishing_tactical_comm,
                                egui::FontId::proportional(18.0),
                                crate::ui::theme::text_secondary(),
                            );
                        });
                    },
                );
            }

            // 3. Bottom Button Area
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                let cancel = crate::widgets::ThemeButton::new("CANCEL")
                    .style(crate::widgets::ThemeButtonStyle::Danger)
                    .min_size(egui::vec2(200.0, action_min_h));
                if ui.add(cancel).clicked() {
                    *action = Some(UiAction::LeaveLobby);
                }
            });
            ui.add_space(16.0);
        });
    });
}

fn draw_map_briefing(
    ui: &mut Ui,
    lobby: &sow_core::protocol::LobbyInfo,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    is_mobile: bool,
    lang: sow_lang::Language,
) {
    let strings = &sow_lang::get(lang).main_menu;
    Frame::NONE
        .fill(crate::ui::theme::nickname_field_bg())
        .stroke(Stroke::new(
            1.0_f32,
            crate::ui::theme::nickname_field_border(),
        ))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(16.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !is_mobile {
                ui.set_height(ui.available_height());
            }
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.vertical(|ui| {
                // Header
                ui.label(
                    RichText::new(&strings.tactical_briefing)
                        .size(14.0)
                        .strong()
                        .color(crate::ui::theme::text_secondary()),
                );
                ui.add_space(4.0);

                // Map Preview Visual
                let thumbnail = asset_loader.thumbnail(&lobby.map_name);
                let aspect = if is_mobile { 2.4_f32 } else { 1.77_f32 }; // Panoramic on mobile
                let preview_w = ui.available_width();
                let max_img_h = if is_mobile {
                    100.0f32
                } else {
                    (ui.available_height() - 190.0).max(80.0)
                };
                let preview_h = (preview_w / aspect).min(max_img_h);

                let rect = ui
                    .allocate_exact_size(egui::vec2(preview_w, preview_h), egui::Sense::hover())
                    .0;

                if let Some(tex) = thumbnail {
                    crate::ui::map_texture::draw_map_thumbnail(
                        ui.painter(),
                        tex.id(),
                        rect,
                        1.0,
                    );
                } else {
                    ui.painter()
                        .rect_filled(rect, 8.0, Color32::from_black_alpha(120));
                    crate::ui::theme::outlined_text(
                        ui.painter(),
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        &strings.holographic_scanning,
                        egui::FontId::proportional(14.0),
                        crate::ui::theme::text_secondary(),
                        Color32::BLACK,
                    );
                }

                // Cyber glowing map border
                ui.painter().rect_stroke(
                    rect,
                    8.0,
                    Stroke::new(1.5_f32, crate::ui::theme::menu_panel_border_glow()),
                    egui::StrokeKind::Inside,
                );

                ui.add_space(6.0);

                // Map details
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(lobby.map_name.to_uppercase())
                                .size(if is_mobile { 18.0 } else { 24.0 })
                                .strong()
                                .color(Color32::WHITE),
                        );
                        ui.add_space(2.0);

                        // Mode indicator
                        let (mode_label, mode_color) = if lobby.game_mode == "FFA" {
                            (&strings.free_for_all, crate::ui::theme::accent_solo_cyan())
                        } else if lobby.game_mode == "Teams" {
                            (
                                &strings.team_tactics,
                                crate::ui::theme::accent_ranked_gold(),
                            )
                        } else {
                            (&strings.simulation, crate::ui::theme::avatar_pink())
                        };

                        Frame::NONE
                            .fill(mode_color.linear_multiply(0.15))
                            .stroke(Stroke::new(1.0_f32, mode_color))
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(Margin::symmetric(8, 4))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(mode_label)
                                        .size(12.0)
                                        .strong()
                                        .color(mode_color),
                                );
                            });
                    });
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                // Telemetry Details
                if is_mobile {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&strings.channel)
                                .size(11.0)
                                .color(crate::ui::theme::text_secondary()),
                        );
                        ui.label(
                            RichText::new(format!("#{:06X}", lobby.id % 0xFFFFFF))
                                .size(11.0)
                                .strong()
                                .color(Color32::WHITE),
                        );
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(&strings.slots)
                                .size(11.0)
                                .color(crate::ui::theme::text_secondary()),
                        );
                        ui.label(
                            RichText::new(format!("{}", lobby.max_players))
                                .size(11.0)
                                .strong()
                                .color(Color32::WHITE),
                        );
                    });
                } else {
                    let mut draw_detail = |key: &str, val: &str| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(key)
                                    .size(12.0)
                                    .color(crate::ui::theme::text_secondary()),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(val)
                                            .size(12.0)
                                            .strong()
                                            .color(Color32::WHITE),
                                    );
                                },
                            );
                        });
                        ui.add_space(2.0);
                    };

                    draw_detail(
                        &strings.lobby_channel_label,
                        &format!("#{:06X}", lobby.id % 0xFFFFFF),
                    );
                    draw_detail(
                        &strings.max_sector_slots,
                        &format!("{} PARTICIPANTS", lobby.max_players),
                    );
                    draw_detail(&strings.deployment_engine, &strings.deployment_engine_val);
                }
            });
        });
}

fn draw_ready_room(
    ui: &mut Ui,
    lobby: &sow_core::protocol::LobbyInfo,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_lang::Language,
) {
    let strings = &sow_lang::get(lang).main_menu;
    Frame::NONE
        .fill(crate::ui::theme::nickname_field_bg())
        .stroke(Stroke::new(
            1.0_f32,
            crate::ui::theme::nickname_field_border(),
        ))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(16.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_height(ui.available_height());
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.vertical(|ui| {
                // Header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&strings.ready_room)
                            .size(14.0)
                            .strong()
                            .color(crate::ui::theme::text_secondary()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{}/{}", lobby.num_players, lobby.max_players))
                                .size(14.0)
                                .strong()
                                .color(Color32::WHITE),
                        );
                    });
                });
                ui.add_space(12.0);

                // Player List Scrollable
                let remaining_h = ui.available_height() - 8.0;
                egui::ScrollArea::vertical()
                    .max_height(remaining_h)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 8.0);
                        for p in &lobby.players {
                            Frame::NONE
                                .fill(crate::ui::theme::panel_bg_transparent())
                                .stroke(Stroke::new(
                                    1.0_f32,
                                    crate::ui::theme::nickname_field_border(),
                                ))
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(Margin::symmetric(12, 10))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        // 1. Chosen Leader Avatar
                                        let avatar_tex = asset_loader
                                            .avatars
                                            .get(&p.leader)
                                            .or(asset_loader.avatar_fallback.as_ref());
                                        if let Some(tex) = avatar_tex {
                                            ui.add(
                                                egui::Image::new(tex)
                                                    .fit_to_exact_size(egui::vec2(28.0, 28.0))
                                                    .corner_radius(CornerRadius::same(14)),
                                            );
                                        }

                                        ui.add_space(8.0);

                                        // 2. Player Name
                                        ui.label(
                                            RichText::new(&p.name)
                                                .size(16.0)
                                                .strong()
                                                .color(Color32::WHITE),
                                        );

                                        // 3. Ready Badge
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                let map_ready =
                                                    p.download_progress == 100 || p.is_ready;
                                                if map_ready {
                                                    Frame::NONE
                                                        .fill(Color32::from_rgba_unmultiplied(
                                                            74, 222, 128, 30,
                                                        ))
                                                        .stroke(Stroke::new(
                                                            1.0_f32,
                                                            Color32::from_rgb(74, 222, 128),
                                                        ))
                                                        .corner_radius(CornerRadius::same(4))
                                                        .inner_margin(Margin::symmetric(8, 4))
                                                        .show(ui, |ui| {
                                                            ui.label(
                                                                RichText::new(&strings.ready)
                                                                    .size(11.0)
                                                                    .strong()
                                                                    .color(Color32::from_rgb(
                                                                        74, 222, 128,
                                                                    )),
                                                            );
                                                        });
                                                } else {
                                                    Frame::NONE
                                                        .fill(Color32::from_rgba_unmultiplied(
                                                            250, 204, 21, 30,
                                                        ))
                                                        .stroke(Stroke::new(
                                                            1.0_f32,
                                                            Color32::from_rgb(250, 204, 21),
                                                        ))
                                                        .corner_radius(CornerRadius::same(4))
                                                        .inner_margin(Margin::symmetric(8, 4))
                                                        .show(ui, |ui| {
                                                            ui.label(
                                                                RichText::new(format!(
                                                                    "SYNCING {}%",
                                                                    p.download_progress
                                                                ))
                                                                .size(11.0)
                                                                .strong()
                                                                .color(Color32::from_rgb(
                                                                    250, 204, 21,
                                                                )),
                                                            );
                                                        });
                                                }
                                            },
                                        );
                                    });
                                });
                        }
                    });
            });
        });
}
