use crate::UiAction;
use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, Ui};
use super::MainMenuState;

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
    let screen_rect = ui.ctx().content_rect();
    let screen_w = screen_rect.width();
    let screen_h = screen_rect.height();
    let is_mobile = screen_w < 900.0;

    // 1. Fullscreen Background Image
    let background_tex = if is_mobile {
        asset_loader.splash_mobile.as_ref()
    } else {
        asset_loader.splash_desktop.as_ref()
    };

    if let Some(texture) = background_tex {
        let tex_aspect = texture.size()[0] as f32 / texture.size()[1] as f32;
        let screen_aspect = screen_w / screen_h;

        let (mut u0, mut v0, mut u1, mut v1) = (0.0, 0.0, 1.0, 1.0);

        if tex_aspect > screen_aspect {
            let crop_w = screen_aspect / tex_aspect;
            u0 = (1.0 - crop_w) / 2.0;
            u1 = 1.0 - u0;
        } else {
            let crop_h = tex_aspect / screen_aspect;
            v0 = (1.0 - crop_h) / 2.0;
            v1 = 1.0 - v0;
        }

        ui.painter().image(
            texture.id(),
            screen_rect,
            egui::Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1)),
            Color32::WHITE,
        );
    }

    // 2. Premium dark overlay for contrast
    ui.painter().rect_filled(
        screen_rect,
        0.0,
        Color32::from_rgba_unmultiplied(8, 12, 24, 215),
    );

    // Get lobby information
    let mut lobby_info = None;
    if let Some(lobby_id) = state.joined_lobby_id.or(state.pending_join_lobby_id) {
        if let Some(lobby) = state.lobbies.iter().find(|l| l.id == lobby_id) {
            lobby_info = Some(lobby);
        }
    }

    // 3. Main Center Container Frame
    let container_w = if is_mobile {
        screen_w - 32.0
    } else {
        880.0f32.min(screen_w - 64.0)
    };
    let container_h = if is_mobile {
        screen_h - 32.0
    } else {
        720.0f32.min(screen_h - 100.0)
    };

    let center_rect = egui::Rect::from_center_size(screen_rect.center(), egui::vec2(container_w, container_h));

    ui.scope_builder(egui::UiBuilder::new().max_rect(center_rect), |ui| {
        Frame::NONE
            .fill(crate::ui::theme::panel_bg())
            .stroke(Stroke::new(1.5_f32, crate::ui::theme::menu_panel_border_glow()))
            .corner_radius(CornerRadius::same(20))
            .inner_margin(if is_mobile { 16.0 } else { 28.0 })
            .shadow(egui::Shadow {
                blur: 32,
                spread: 0,
                color: crate::ui::theme::menu_panel_border_glow().linear_multiply(0.25),
                offset: [0, 10],
            })
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    let button_h = action_min_h + 16.0;
                    let top_h = ui.available_height() - button_h;

                    // 1. Top Area (All content except the leave button)
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), top_h),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            if let Some(lobby) = lobby_info {
                                // Title / Status row
                                ui.vertical_centered(|ui| {
                                    crate::ui::theme::outlined_label(
                                        ui,
                                        &strings.matchmaking_established,
                                        egui::FontId::proportional(if is_mobile { 24.0 } else { 32.0 }),
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

                                    ui.add_space(4.0);
                                    crate::ui::theme::outlined_label(
                                        ui,
                                        &timer_text,
                                        egui::FontId::proportional(if is_mobile { 16.0 } else { 20.0 }),
                                        timer_color,
                                    );
                                });

                                ui.add_space(section_gap * 1.5_f32);

                                // Dual Columns or Stack
                                if is_mobile {
                                    // On mobile, vertical non-scrolling layout
                                    ui.vertical(|ui| {
                                        draw_map_briefing(ui, lobby, asset_loader, is_mobile, lang);
                                        ui.add_space(12.0);
                                        ui.allocate_ui_with_layout(
                                            egui::vec2(ui.available_width(), ui.available_height()),
                                            egui::Layout::top_down(egui::Align::Min),
                                            |ui| {
                                                draw_ready_room(ui, lobby, asset_loader, lang);
                                            },
                                        );
                                    });
                                } else {
                                    // Desktop side-by-side
                                    ui.horizontal_top(|ui| {
                                        let total_w = ui.available_width();
                                        let col_w = (total_w - 24.0) * 0.5_f32;

                                        ui.allocate_ui_with_layout(
                                            egui::vec2(col_w, ui.available_height()),
                                            egui::Layout::top_down(egui::Align::Min),
                                            |ui| {
                                                draw_map_briefing(ui, lobby, asset_loader, is_mobile, lang);
                                            },
                                        );

                                        ui.add_space(24.0);

                                        ui.allocate_ui_with_layout(
                                            egui::vec2(col_w, ui.available_height()),
                                            egui::Layout::top_down(egui::Align::Min),
                                            |ui| {
                                                draw_ready_room(ui, lobby, asset_loader, lang);
                                            },
                                        );
                                    });
                                }
                            } else {
                                // Lobby sync loading
                                ui.vertical_centered(|ui| {
                                    ui.add_space(80.0);
                                    ui.add(egui::Spinner::new().size(40.0));
                                    ui.add_space(16.0);
                                    crate::ui::theme::outlined_label(
                                        ui,
                                        &strings.establishing_tactical_comm,
                                        egui::FontId::proportional(20.0),
                                        crate::ui::theme::text_secondary(),
                                    );
                                });
                            }
                        },
                    );

                    // 2. Bottom Area (Leave button)
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        let cancel = crate::widgets::ThemeButton::new(&strings.leave_lobby)
                            .style(crate::widgets::ThemeButtonStyle::Danger)
                            .min_size(egui::vec2(220.0, action_min_h));
                        if ui.add(cancel).clicked() {
                            *action = Some(UiAction::LeaveLobby);
                        }
                    });
                });
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
        .stroke(Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border()))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(16.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical(|ui| {
                // Header
                ui.label(RichText::new(&strings.tactical_briefing).size(14.0).strong().color(crate::ui::theme::text_secondary()));
                ui.add_space(8.0);

                // Map Preview Visual
                let thumbnail = asset_loader.thumbnail(&lobby.map_name);
                let aspect = if is_mobile { 2.4_f32 } else { 1.77_f32 }; // Panoramic on mobile
                let preview_w = ui.available_width();
                let preview_h = if is_mobile { 100.0f32.min(preview_w / aspect) } else { preview_w / aspect };

                let rect = ui.allocate_exact_size(egui::vec2(preview_w, preview_h), egui::Sense::hover()).0;

                if let Some(tex) = thumbnail {
                    let image = egui::Image::new(tex)
                        .fit_to_exact_size(rect.size())
                        .corner_radius(CornerRadius::same(8));
                    ui.put(rect, image);
                } else {
                    ui.painter().rect_filled(rect, 8.0, Color32::from_black_alpha(120));
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

                ui.add_space(12.0);

                // Map details
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(lobby.map_name.to_uppercase()).size(if is_mobile { 18.0 } else { 24.0 }).strong().color(Color32::WHITE));
                        ui.add_space(4.0);

                        // Mode indicator
                        let (mode_label, mode_color) = if lobby.game_mode == "FFA" {
                            (&strings.free_for_all, crate::ui::theme::accent_solo_cyan())
                        } else if lobby.game_mode == "Teams" {
                            (&strings.team_tactics, crate::ui::theme::accent_ranked_gold())
                        } else {
                            (&strings.simulation, crate::ui::theme::avatar_pink())
                        };

                        Frame::NONE
                            .fill(mode_color.linear_multiply(0.15))
                            .stroke(Stroke::new(1.0_f32, mode_color))
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(Margin::symmetric(8, 4))
                            .show(ui, |ui| {
                                ui.label(RichText::new(mode_label).size(12.0).strong().color(mode_color));
                            });
                    });
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // Telemetry Details
                if is_mobile {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&strings.channel).size(11.0).color(crate::ui::theme::text_secondary()));
                        ui.label(RichText::new(format!("#{:06X}", lobby.id % 0xFFFFFF)).size(11.0).strong().color(Color32::WHITE));
                        ui.add_space(12.0);
                        ui.label(RichText::new(&strings.slots).size(11.0).color(crate::ui::theme::text_secondary()));
                        ui.label(RichText::new(format!("{}", lobby.max_players)).size(11.0).strong().color(Color32::WHITE));
                    });
                } else {
                    let mut draw_detail = |key: &str, val: &str| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(key).size(12.0).color(crate::ui::theme::text_secondary()));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(val).size(12.0).strong().color(Color32::WHITE));
                            });
                        });
                        ui.add_space(4.0);
                    };

                    draw_detail(&strings.lobby_channel_label, &format!("#{:06X}", lobby.id % 0xFFFFFF));
                    draw_detail(&strings.max_sector_slots, &format!("{} PARTICIPANTS", lobby.max_players));
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
        .stroke(Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border()))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(16.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_height(ui.available_height());
            ui.vertical(|ui| {
                // Header
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&strings.ready_room).size(14.0).strong().color(crate::ui::theme::text_secondary()));
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
                                .stroke(Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border()))
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(Margin::symmetric(12, 10))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        // 1. Name Hash Avatar
                                        let name_hash = p.name.chars().map(|c| c as usize).sum::<usize>();
                                        let avatar_idx = name_hash % 8;
                                        let avatar_tex = asset_loader.avatars.get(avatar_idx).or(asset_loader.avatar_fallback.as_ref());
                                        if let Some(tex) = avatar_tex {
                                            ui.add(egui::Image::new(tex)
                                                .fit_to_exact_size(egui::vec2(28.0, 28.0))
                                                .corner_radius(CornerRadius::same(14)));
                                        }

                                        ui.add_space(8.0);

                                        // 2. Player Name
                                        ui.label(RichText::new(&p.name).size(16.0).strong().color(Color32::WHITE));

                                        // 3. Ready Badge
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            let map_ready = p.download_progress == 100 || p.is_ready;
                                            if map_ready {
                                                Frame::NONE
                                                    .fill(Color32::from_rgba_unmultiplied(74, 222, 128, 30))
                                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(74, 222, 128)))
                                                    .corner_radius(CornerRadius::same(4))
                                                    .inner_margin(Margin::symmetric(8, 4))
                                                    .show(ui, |ui| {
                                                        ui.label(RichText::new(&strings.ready).size(11.0).strong().color(Color32::from_rgb(74, 222, 128)));
                                                    });
                                            } else {
                                                Frame::NONE
                                                    .fill(Color32::from_rgba_unmultiplied(250, 204, 21, 30))
                                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(250, 204, 21)))
                                                    .corner_radius(CornerRadius::same(4))
                                                    .inner_margin(Margin::symmetric(8, 4))
                                                    .show(ui, |ui| {
                                                        ui.label(RichText::new(format!("SYNCING {}%", p.download_progress)).size(11.0).strong().color(Color32::from_rgb(250, 204, 21)));
                                                    });
                                            }
                                        });
                                    });
                                });
                        }
                    });
            });
        });
}
