use super::MainMenuState;
use crate::UiAction;
use egui::{Align, Color32, Context, CornerRadius, Frame, Layout, Margin, RichText, Stroke, Ui};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LobbyBodyLayout {
    FullScreen,
    ModalStack,
}

/// Snapshot of who may moderate the roster — lets `draw_ready_room` render the
/// host's Kick/Ban controls without holding a borrow on `MainMenuState`.
#[derive(Clone, Copy)]
struct HostControls {
    is_host: bool,
    my_player_id: Option<u16>,
    lobby_id: u64,
}

impl HostControls {
    fn from_state(state: &MainMenuState, lobby_id: u64) -> Self {
        Self {
            is_host: state.is_lobby_host,
            my_player_id: state.my_player_id,
            lobby_id,
        }
    }
}

fn resolve_lobby_info(state: &MainMenuState) -> Option<&sow_core::protocol::LobbyInfo> {
    let lobby_id = state.joined_lobby_id.or(state.pending_join_lobby_id)?;
    state.lobbies.iter().find(|l| l.id == lobby_id)
}

fn lobby_bottom_action_height(
    compact: bool,
    action_min_h: f32,
    show_invite: bool,
    show_start: bool,
) -> f32 {
    if compact {
        let rows = 1.0 + if show_invite { 1.0 } else { 0.0 } + if show_start { 1.0 } else { 0.0 };
        rows * action_min_h + (rows - 1.0) * 8.0 + 24.0
    } else {
        action_min_h + 24.0
    }
}

fn lobby_action_flags(
    state: &MainMenuState,
    lobby_info: Option<&sow_core::protocol::LobbyInfo>,
) -> (bool, bool) {
    let show_invite = state.in_private_match && lobby_info.is_some();
    let show_start = state.in_private_match && state.is_lobby_host && lobby_info.is_some();
    (show_invite, show_start)
}

fn draw_lobby_bottom_actions(
    ui: &mut Ui,
    state: &MainMenuState,
    lobby_info: Option<&sow_core::protocol::LobbyInfo>,
    action_min_h: f32,
    action: &mut Option<UiAction>,
    lang: sow_i18n::Language,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    if state.is_lobby_host {
        if let Some(lobby) = lobby_info {
            let start_btn = crate::widgets::ThemeButton::new(&strings.start_game)
                .style(crate::widgets::ThemeButtonStyle::Primary)
                .min_size(egui::vec2(200.0, action_min_h));
            if ui.add(start_btn).clicked() {
                *action = Some(UiAction::StartPrivateLobby(lobby.id));
            }
            ui.add_space(8.0);
        }
    }
    if state.in_private_match {
        if let Some(lobby) = lobby_info {
            let now = ui.input(|i| i.time);
            let is_copied = if let Some(t) = state.invite_copied_at {
                now - t < 2.0
            } else {
                false
            };
            let label = if is_copied {
                &strings.invite_link_copied
            } else {
                &strings.copy_invite_link
            };
            let invite_btn = crate::widgets::ThemeButton::new(label)
                .style(crate::widgets::ThemeButtonStyle::Secondary)
                .min_size(egui::vec2(200.0, action_min_h));
            if ui.add(invite_btn).clicked() {
                *action = Some(UiAction::CopyInviteLink(lobby.id));
            }
            ui.add_space(8.0);
        }
    }

    let cancel = crate::widgets::ThemeButton::new(&strings.leave_lobby)
        .style(crate::widgets::ThemeButtonStyle::Danger)
        .min_size(egui::vec2(200.0, action_min_h));
    if ui.add(cancel).clicked() {
        *action = Some(UiAction::LeaveLobby);
    }
}

fn draw_lobby_header(
    ui: &mut Ui,
    state: &MainMenuState,
    lobby: &sow_core::protocol::LobbyInfo,
    section_gap: f32,
    lang: sow_i18n::Language,
    compact: bool,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    ui.vertical_centered(|ui| {
        sow_ui_kit::theme::outlined_label(
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
            sow_ui_kit::theme::palette::text_muted()
        };

        ui.add_space(2.0);
        sow_ui_kit::theme::outlined_label(
            ui,
            &timer_text,
            egui::FontId::proportional(if compact { 14.0 } else { 18.0 }),
            timer_color,
        );
    });
    ui.add_space(section_gap);
}

fn draw_lobby_connecting(ui: &mut Ui, middle_h: f32, lang: sow_i18n::Language) {
    let strings = &sow_i18n::get(lang).main_menu;
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), middle_h),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(middle_h * 0.35);
                ui.add(egui::Spinner::new().size(36.0));
                ui.add_space(16.0);
                sow_ui_kit::theme::outlined_label(
                    ui,
                    &strings.establishing_tactical_comm,
                    egui::FontId::proportional(18.0),
                    sow_ui_kit::theme::palette::text_muted(),
                );
            });
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_lobby_body(
    ui: &mut Ui,
    lobby: &sow_core::protocol::LobbyInfo,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    compact: bool,
    layout: LobbyBodyLayout,
    middle_h: f32,
    host: HostControls,
    action: &mut Option<UiAction>,
) {
    match layout {
        LobbyBodyLayout::ModalStack => {
            ui.vertical(|ui| {
                draw_map_briefing(ui, lobby, asset_loader, true, lang);
                ui.add_space(8.0);
                draw_ready_room(ui, lobby, asset_loader, lang, host, action);
            });
        }
        LobbyBodyLayout::FullScreen => {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), middle_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    if compact {
                        ui.vertical(|ui| {
                            draw_map_briefing(ui, lobby, asset_loader, true, lang);
                            ui.add_space(8.0);
                            let ready_room_h = ui.available_height();
                            ui.allocate_ui_with_layout(
                                egui::vec2(ui.available_width(), ready_room_h),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    draw_ready_room(ui, lobby, asset_loader, lang, host, action);
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
                                    draw_ready_room(ui, lobby, asset_loader, lang, host, action);
                                },
                            );
                        });
                    }
                },
            );
        }
    }
}

fn draw_lobby_footer(
    ui: &mut Ui,
    state: &MainMenuState,
    lobby_info: Option<&sow_core::protocol::LobbyInfo>,
    action_min_h: f32,
    action: &mut Option<UiAction>,
    lang: sow_i18n::Language,
    compact: bool,
) {
    ui.add_space(8.0);
    if compact {
        ui.vertical_centered(|ui| {
            draw_lobby_bottom_actions(ui, state, lobby_info, action_min_h, action, lang);
        });
    } else {
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            draw_lobby_bottom_actions(ui, state, lobby_info, action_min_h, action, lang);
        });
    }
    ui.add_space(16.0);
}

pub fn draw_queue_overlay(
    ui: &mut Ui,
    state: &MainMenuState,
    section_gap: f32,
    action_min_h: f32,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
) {
    let compact = sow_ui_kit::theme::compact_viewport(ui.ctx());
    let lobby_info = resolve_lobby_info(state);
    let (show_invite, show_start) = lobby_action_flags(state, lobby_info);

    let panel_frame = sow_ui_kit::theme::standard_panel_frame(compact);
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
                draw_lobby_header(ui, state, lobby, section_gap, lang, compact);

                let button_h =
                    lobby_bottom_action_height(compact, action_min_h, show_invite, show_start);
                let middle_h = ui.available_height() - button_h;
                draw_lobby_body(
                    ui,
                    lobby,
                    asset_loader,
                    lang,
                    compact,
                    LobbyBodyLayout::FullScreen,
                    middle_h,
                    HostControls::from_state(state, lobby.id),
                    action,
                );
            } else {
                let button_h =
                    lobby_bottom_action_height(compact, action_min_h, show_invite, show_start);
                let middle_h = ui.available_height() - button_h;
                draw_lobby_connecting(ui, middle_h, lang);
            }

            draw_lobby_footer(ui, state, lobby_info, action_min_h, action, lang, compact);
        });
    });
}

pub fn draw_lobby_embed_modal(
    ctx: &Context,
    state: &MainMenuState,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    reduced_motion: bool,
) {
    let compact = sow_ui_kit::theme::compact_viewport(ctx);
    let lobby_info = resolve_lobby_info(state);
    let (show_invite, show_start) = lobby_action_flags(state, lobby_info);
    let action_min_h = (if compact { 44.0 } else { 48.0 }) * sow_ui_kit::theme::viewport_scale(ctx);
    let section_gap = 8.0 * sow_ui_kit::theme::viewport_scale(ctx);

    let progress = ctx.animate_bool_with_time(
        egui::Id::new("lobby_embed_modal_animation_progress"),
        true,
        sow_ui_kit::theme::anim_duration(reduced_motion),
    );
    if progress <= 0.01 {
        return;
    }

    let screen_rect = ctx.input(|i| i.content_rect());
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("lobby_embed_modal_scrim"),
    ))
    .rect_filled(
        screen_rect,
        0.0,
        Color32::from_black_alpha((150.0 * progress) as u8),
    );

    let modal_w = (screen_rect.width() - 24.0).clamp(280.0, 520.0);
    let modal_h = (screen_rect.height() - 24.0).clamp(260.0, 520.0);
    let footer_h = lobby_bottom_action_height(compact, action_min_h, show_invite, show_start);
    let header_h = if compact { 56.0 } else { 72.0 };
    let y_offset = if progress >= 1.0 {
        0.0
    } else {
        -80.0 * (1.0 - progress)
    };

    egui::Window::new("lobby_embed_modal")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, y_offset))
        .fixed_size(egui::vec2(modal_w, modal_h))
        .frame(sow_ui_kit::theme::standard_panel_frame(compact))
        .show(ctx, |ui| {
            if let Some(lobby) = lobby_info {
                draw_lobby_header(ui, state, lobby, section_gap, lang, compact);
            } else {
                ui.vertical_centered(|ui| {
                    ui.add(egui::Spinner::new().size(36.0));
                    ui.add_space(8.0);
                    let strings = &sow_i18n::get(lang).main_menu;
                    sow_ui_kit::theme::outlined_label(
                        ui,
                        &strings.establishing_tactical_comm,
                        egui::FontId::proportional(16.0),
                        sow_ui_kit::theme::palette::text_muted(),
                    );
                });
                ui.add_space(section_gap);
            }

            let scroll_h = (modal_h - header_h - footer_h - 24.0).max(80.0);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(scroll_h)
                .show(ui, |ui| {
                    if let Some(lobby) = lobby_info {
                        draw_lobby_body(
                            ui,
                            lobby,
                            asset_loader,
                            lang,
                            compact,
                            LobbyBodyLayout::ModalStack,
                            scroll_h,
                            HostControls::from_state(state, lobby.id),
                            action,
                        );
                    }
                });

            draw_lobby_footer(ui, state, lobby_info, action_min_h, action, lang, compact);
        });
}

fn draw_map_briefing(
    ui: &mut Ui,
    lobby: &sow_core::protocol::LobbyInfo,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    is_mobile: bool,
    lang: sow_i18n::Language,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    Frame::NONE
        .fill(sow_ui_kit::theme::palette::field_bg())
        .stroke(Stroke::new(
            1.0_f32,
            sow_ui_kit::theme::palette::field_border(),
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
                ui.label(
                    RichText::new(&strings.tactical_briefing)
                        .size(14.0)
                        .strong()
                        .color(sow_ui_kit::theme::palette::text_muted()),
                );
                ui.add_space(4.0);

                let thumbnail = asset_loader.thumbnail(&lobby.map_name);
                let aspect = 1.77_f32;
                let preview_w = ui.available_width();
                let max_img_h = if is_mobile {
                    150.0f32
                } else {
                    (ui.available_height() - 190.0).max(80.0)
                };
                let preview_h = (preview_w / aspect).min(max_img_h);

                let rect = ui
                    .allocate_exact_size(egui::vec2(preview_w, preview_h), egui::Sense::hover())
                    .0;

                if let Some(tex) = thumbnail {
                    let uv = crate::ui::map_texture::cover_uv(rect.size(), tex.size_vec2());
                    crate::ui::map_texture::draw_map_thumbnail_uv(
                        ui.painter(),
                        tex.id(),
                        rect,
                        uv,
                        1.0,
                    );
                } else {
                    ui.painter()
                        .rect_filled(rect, 8.0, Color32::from_black_alpha(120));
                    sow_ui_kit::theme::paint_premium_glow_text(
                        ui.painter(),
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        &strings.holographic_scanning,
                        egui::FontId::proportional(14.0),
                        sow_ui_kit::theme::palette::text_muted(),
                        Color32::BLACK,
                    );
                }

                ui.painter().rect_stroke(
                    rect,
                    8.0,
                    Stroke::new(1.5_f32, sow_ui_kit::theme::palette::neon_cyan_glow()),
                    egui::StrokeKind::Inside,
                );

                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(lobby.map_name.to_uppercase())
                                .size(if is_mobile { 18.0 } else { 24.0 })
                                .strong()
                                .color(Color32::WHITE),
                        );
                        ui.add_space(2.0);

                        let (mode_label, mode_color) = if lobby.game_mode == "FFA" {
                            (
                                &strings.free_for_all,
                                sow_ui_kit::theme::palette::neon_cyan(),
                            )
                        } else if lobby.game_mode == "Teams" {
                            (
                                &strings.team_tactics,
                                sow_ui_kit::theme::palette::neon_gold(),
                            )
                        } else {
                            (&strings.simulation, sow_ui_kit::theme::palette::pink())
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

                if is_mobile {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&strings.channel)
                                .size(11.0)
                                .color(sow_ui_kit::theme::palette::text_muted()),
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
                                .color(sow_ui_kit::theme::palette::text_muted()),
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
                                    .color(sow_ui_kit::theme::palette::text_muted()),
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
    lang: sow_i18n::Language,
    host: HostControls,
    action: &mut Option<UiAction>,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    Frame::NONE
        .fill(sow_ui_kit::theme::palette::field_bg())
        .stroke(Stroke::new(
            1.0_f32,
            sow_ui_kit::theme::palette::field_border(),
        ))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(16.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&strings.ready_room)
                            .size(14.0)
                            .strong()
                            .color(sow_ui_kit::theme::palette::text_muted()),
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

                let remaining_h = ui.available_height().max(120.0);
                egui::ScrollArea::vertical()
                    .max_height(remaining_h)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 8.0);
                        for p in &lobby.players {
                            Frame::NONE
                                .fill(sow_ui_kit::theme::palette::surface_transparent())
                                .stroke(Stroke::new(
                                    1.0_f32,
                                    sow_ui_kit::theme::palette::field_border(),
                                ))
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(Margin::symmetric(12, 10))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
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

                                        ui.label(
                                            RichText::new(&p.name)
                                                .size(16.0)
                                                .strong()
                                                .color(Color32::WHITE),
                                        );

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                // Host moderation: kick (rejoinable) / ban
                                                // (blocked) every roster entry but itself.
                                                if host.is_host
                                                    && Some(p.player_id) != host.my_player_id
                                                {
                                                    let ban_btn = crate::widgets::ThemeButton::new(
                                                        &strings.ban_btn,
                                                    )
                                                    .style(crate::widgets::ThemeButtonStyle::Danger)
                                                    .text_size(11.0)
                                                    .min_size(egui::vec2(0.0, 24.0));
                                                    if ui.add(ban_btn).clicked() {
                                                        *action = Some(UiAction::BanPlayer {
                                                            lobby_id: host.lobby_id,
                                                            target_player_id: p.player_id,
                                                        });
                                                    }
                                                    ui.add_space(4.0);
                                                    let kick_btn = crate::widgets::ThemeButton::new(
                                                        &strings.kick_btn,
                                                    )
                                                    .style(
                                                        crate::widgets::ThemeButtonStyle::Secondary,
                                                    )
                                                    .text_size(11.0)
                                                    .min_size(egui::vec2(0.0, 24.0));
                                                    if ui.add(kick_btn).clicked() {
                                                        *action = Some(UiAction::KickPlayer {
                                                            lobby_id: host.lobby_id,
                                                            target_player_id: p.player_id,
                                                        });
                                                    }
                                                    ui.add_space(8.0);
                                                }
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
