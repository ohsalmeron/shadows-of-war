use super::MainMenuState;
use crate::UiAction;
use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, Ui};

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
    middle_h: f32,
    host: HostControls,
    action: &mut Option<UiAction>,
) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), middle_h),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            if compact {
                // ponytail: single scroll area bounds all middle content to middle_h;
                // footer buttons always stay anchored below.
                egui::ScrollArea::vertical()
                    .id_salt("lobby_body_compact")
                    .max_height(middle_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        draw_map_briefing(ui, lobby, asset_loader, true, lang);
                        ui.add_space(8.0);
                        draw_ready_room(ui, lobby, asset_loader, lang, host, action);
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

fn draw_lobby_footer(
    ui: &mut Ui,
    state: &MainMenuState,
    lobby_info: Option<&sow_core::protocol::LobbyInfo>,
    action_min_h: f32,
    action: &mut Option<UiAction>,
    lang: sow_i18n::Language,
    compact: bool,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    let now = ui.input(|i| i.time);
    let is_copied = if let Some(t) = state.invite_copied_at {
        now - t < 2.0
    } else {
        false
    };

    if compact {
        // Row 1: Cancel (left, red) + Copy/Invite (right, green/secondary)
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let half_w = (ui.available_width() - 8.0) / 2.0;

            // Cancel — red, left
            let cancel_btn = crate::widgets::ThemeButton::new(&strings.leave_lobby)
                .style(crate::widgets::ThemeButtonStyle::Danger)
                .min_size(egui::vec2(half_w, action_min_h))
                .text_size(13.0);
            if ui.add(cancel_btn).clicked() {
                *action = Some(UiAction::LeaveLobby);
            }

            // Copy/Invite — secondary/green, right
            if let Some(lobby) = lobby_info {
                let label = if is_copied {
                    &strings.invite_link_copied
                } else {
                    &strings.copy_invite_link
                };
                let copy_btn = crate::widgets::ThemeButton::new(label)
                    .style(crate::widgets::ThemeButtonStyle::Secondary)
                    .min_size(egui::vec2(half_w, action_min_h))
                    .text_size(13.0);
                if ui.add(copy_btn).clicked() {
                    *action = Some(UiAction::CopyInviteLink(lobby.id));
                }
            } else {
                // No lobby yet — spacer to keep cancel on the left
                ui.allocate_space(egui::vec2(half_w, 0.0));
            }
        });

        // Row 2: Start Game (full width, only for host)
        if state.is_lobby_host {
            if let Some(lobby) = lobby_info {
                ui.add_space(8.0);
                let start_btn = crate::widgets::ThemeButton::new(&strings.start_game)
                    .style(crate::widgets::ThemeButtonStyle::Primary)
                    .min_size(egui::vec2(ui.available_width(), action_min_h))
                    .text_size(15.0);
                if ui.add(start_btn).clicked() {
                    *action = Some(UiAction::StartPrivateLobby(lobby.id));
                }
            }
        }
    } else {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
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
                // Respect the map's real aspect ratio (from the loaded thumbnail) instead
                // of forcing a fixed 16:9 crop; fall back to a neutral ratio until loaded.
                let aspect = thumbnail
                    .map(|t| {
                        let s = t.size_vec2();
                        if s.y > 0.0 {
                            (s.x / s.y).clamp(0.5, 3.0)
                        } else {
                            1.6
                        }
                    })
                    .unwrap_or(1.6);
                let preview_w = ui.available_width();
                let max_img_h = if is_mobile {
                    180.0f32
                } else {
                    (ui.available_height() - 210.0).max(120.0)
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
                        &strings.loading_thumbnail,
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

                // Server-truth only: every row below comes straight from `LobbyInfo`.
                let mut draw_detail = |key: &str, val: String| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(key)
                                .size(12.0)
                                .color(sow_ui_kit::theme::palette::text_muted()),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(val).size(12.0).strong().color(Color32::WHITE));
                        });
                    });
                    ui.add_space(2.0);
                };

                if !lobby.host_name.is_empty() {
                    draw_detail(&strings.host_label, lobby.host_name.clone());
                }
                draw_detail(
                    &strings.players_label,
                    format!("{}/{}", lobby.num_players, lobby.max_players),
                );
                if lobby.bot_count > 0 {
                    draw_detail(&strings.bots_label, lobby.bot_count.to_string());
                    draw_detail(
                        &strings.bot_difficulty,
                        format!("{:?}", lobby.bot_difficulty),
                    );
                }
                if lobby.nation_count > 0 {
                    draw_detail(&strings.nations_count, lobby.nation_count.to_string());
                }
                if lobby.has_password {
                    draw_detail(&strings.password_label, "🔒".to_string());
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
    let is_teams = lobby.game_mode == "Teams";

    // Which roster entry's action menu is open (ephemeral UI state in egui memory).
    let menu_id = egui::Id::new("lobby_roster_action_menu");
    let menu_open: Option<u16> = ui
        .ctx()
        .data(|d| d.get_temp::<Option<u16>>(menu_id))
        .flatten();
    let mut new_menu_open = menu_open;
    let mut name_click: Option<u16> = None;
    let mut click_elsewhere = false;

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
                            // Host can act on everyone but itself.
                            let can_moderate =
                                host.is_host && Some(p.player_id) != host.my_player_id;

                            let row = Frame::NONE
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

                                        // BroodWar-style: click the nameplate to open the
                                        // host action menu (kick / move team / ban).
                                        let sense = if can_moderate {
                                            egui::Sense::click()
                                        } else {
                                            egui::Sense::hover()
                                        };
                                        let mut name_resp = ui.add(
                                            egui::Label::new(
                                                RichText::new(&p.name)
                                                    .size(16.0)
                                                    .strong()
                                                    .color(Color32::WHITE),
                                            )
                                            .sense(sense),
                                        );
                                        if can_moderate {
                                            name_resp = name_resp
                                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                                            if name_resp.clicked() {
                                                name_click = Some(p.player_id);
                                            }
                                        }

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                let map_ready =
                                                    p.download_progress == 100 || p.is_ready;
                                                if map_ready {
                                                    status_badge(
                                                        ui,
                                                        &strings.ready,
                                                        Color32::from_rgb(74, 222, 128),
                                                    );
                                                } else {
                                                    status_badge(
                                                        ui,
                                                        &format!("SYNCING {}%", p.download_progress),
                                                        Color32::from_rgb(250, 204, 21),
                                                    );
                                                }
                                                if is_teams {
                                                    if let Some((label, col)) = team_chip(p.team) {
                                                        ui.add_space(6.0);
                                                        status_badge(ui, label, col);
                                                    }
                                                }
                                            },
                                        );

                                        name_resp.rect
                                    })
                                    .inner
                                });

                            // Floating action menu anchored under the nameplate. Drawn as a
                            // foreground Area so it escapes the roster scroll clip.
                            if menu_open == Some(p.player_id) && can_moderate {
                                let area = egui::Area::new(egui::Id::new((
                                    "lobby_roster_menu",
                                    p.player_id,
                                )))
                                .order(egui::Order::Foreground)
                                .fixed_pos(row.inner.left_bottom() + egui::vec2(0.0, 4.0))
                                .show(ui.ctx(), |ui| {
                                    egui::Frame::menu(&ui.ctx().global_style())
                                        .fill(Color32::from_black_alpha(235))
                                        .stroke(Stroke::new(
                                            1.5_f32,
                                            sow_ui_kit::theme::palette::neon_cyan(),
                                        ))
                                        .corner_radius(8)
                                        .inner_margin(Margin::symmetric(6, 6))
                                        .show(ui, |ui| {
                                            ui.set_min_width(140.0);
                                            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                            let bw = 132.0_f32;
                                            if ui
                                                .add(
                                                    crate::widgets::ThemeButton::new(
                                                        &strings.kick_btn,
                                                    )
                                                    .style(crate::widgets::ThemeButtonStyle::Secondary)
                                                    .text_size(12.0)
                                                    .min_size(egui::vec2(bw, 28.0)),
                                                )
                                                .clicked()
                                            {
                                                *action = Some(UiAction::KickPlayer {
                                                    lobby_id: host.lobby_id,
                                                    target_player_id: p.player_id,
                                                });
                                                new_menu_open = None;
                                            }
                                            if is_teams
                                                && ui
                                                    .add(
                                                        crate::widgets::ThemeButton::new(
                                                            &strings.move_team_btn,
                                                        )
                                                        .style(
                                                            crate::widgets::ThemeButtonStyle::Secondary,
                                                        )
                                                        .text_size(12.0)
                                                        .min_size(egui::vec2(bw, 28.0)),
                                                    )
                                                    .clicked()
                                            {
                                                *action = Some(UiAction::MovePlayerTeam {
                                                    lobby_id: host.lobby_id,
                                                    target_player_id: p.player_id,
                                                });
                                                new_menu_open = None;
                                            }
                                            if ui
                                                .add(
                                                    crate::widgets::ThemeButton::new(
                                                        &strings.ban_btn,
                                                    )
                                                    .style(crate::widgets::ThemeButtonStyle::Danger)
                                                    .text_size(12.0)
                                                    .min_size(egui::vec2(bw, 28.0)),
                                                )
                                                .clicked()
                                            {
                                                *action = Some(UiAction::BanPlayer {
                                                    lobby_id: host.lobby_id,
                                                    target_player_id: p.player_id,
                                                });
                                                new_menu_open = None;
                                            }
                                        });
                                });
                                if area.response.clicked_elsewhere() {
                                    click_elsewhere = true;
                                }
                            }
                        }
                    });
            });
        });

    // Resolve the open menu after the loop so a fresh nameplate click always wins over
    // the outside-click dismissal of whichever menu was previously open.
    if let Some(pid) = name_click {
        new_menu_open = if menu_open == Some(pid) {
            None
        } else {
            Some(pid)
        };
    } else if click_elsewhere {
        new_menu_open = None;
    }
    if new_menu_open != menu_open {
        ui.ctx().data_mut(|d| d.insert_temp(menu_id, new_menu_open));
    }
}

/// Small pill badge (ready / syncing / team) tinted with `color`.
fn status_badge(ui: &mut Ui, label: &str, color: Color32) {
    Frame::NONE
        .fill(color.linear_multiply(0.12))
        .stroke(Stroke::new(1.0_f32, color))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(11.0).strong().color(color));
        });
}

/// Label + color for a player's lobby team chip, or `None` if unassigned.
fn team_chip(team: Option<sow_core::protocol::Team>) -> Option<(&'static str, Color32)> {
    match team {
        Some(sow_core::protocol::Team::Red) => Some(("RED", Color32::from_rgb(239, 68, 68))),
        Some(sow_core::protocol::Team::Blue) => Some(("BLUE", Color32::from_rgb(59, 130, 246))),
        None => None,
    }
}
