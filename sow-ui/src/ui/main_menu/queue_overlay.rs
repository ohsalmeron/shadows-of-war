use super::MainMenuState;
use crate::UiAction;
use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, Ui};

const COLUMN_GAP: f32 = 20.0;
const ACTION_GAP: f32 = 8.0;
/// Gap between the scrollable/clipped content region and the pinned action row.
const ACTIONS_TOP_GAP: f32 = 12.0;
/// Breathing room below the pinned buttons on compact/mobile so they never touch
/// (or clip against) the card's bottom edge / OS gesture bar.
const ACTIONS_BOTTOM_PAD: f32 = 12.0;

/// Snapshot of who may moderate the roster — lets `draw_players_panel` render the
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

fn is_custom_lobby(lobby: &sow_core::protocol::LobbyInfo) -> bool {
    lobby.kind == sow_core::protocol::LobbyKind::Custom
}

fn lobby_countdown_active(state: &MainMenuState, lobby: &sow_core::protocol::LobbyInfo) -> bool {
    lobby.is_counting_down || state.wait_timer_secs > 0.0
}

fn countdown_label(state: &MainMenuState, lobby: &sow_core::protocol::LobbyInfo) -> String {
    if lobby.is_counting_down {
        format!("STARTING IN: {:.1}S", lobby.timer_secs)
    } else {
        format!("STARTING IN: {:.1}S", state.wait_timer_secs)
    }
}

fn draw_countdown_strip(ui: &mut Ui, label: &str, compact: bool) {
    ui.vertical_centered(|ui| {
        sow_ui_kit::theme::outlined_label(
            ui,
            label,
            egui::FontId::proportional(if compact { 14.0 } else { 16.0 }),
            Color32::from_rgb(255, 210, 120),
        );
    });
    ui.add_space(if compact { 6.0 } else { 8.0 });
}

/// Single button row: `[START][CANCEL]` for the host of a custom lobby,
/// `[CANCEL]` for everyone else. Total height is exactly `action_min_h`.
fn draw_lobby_actions(
    ui: &mut Ui,
    state: &MainMenuState,
    lobby: &sow_core::protocol::LobbyInfo,
    action_min_h: f32,
    action: &mut Option<UiAction>,
    lang: sow_i18n::Language,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    let show_start = state.is_lobby_host && is_custom_lobby(lobby);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = ACTION_GAP;
        let btn_w = if show_start {
            (ui.available_width() - ACTION_GAP) / 2.0
        } else {
            ui.available_width()
        };

        if show_start {
            let start_btn = crate::widgets::ThemeButton::new(&strings.start_game)
                .style(crate::widgets::ThemeButtonStyle::Primary)
                .min_size(egui::vec2(btn_w, action_min_h))
                .text_size(14.0);
            if ui.add(start_btn).clicked() {
                *action = Some(UiAction::StartPrivateLobby(lobby.id));
            }
        }

        let cancel_btn = crate::widgets::ThemeButton::new(&strings.leave_lobby)
            .style(crate::widgets::ThemeButtonStyle::Danger)
            .min_size(egui::vec2(btn_w, action_min_h))
            .text_size(14.0);
        if ui.add(cancel_btn).clicked() {
            *action = Some(UiAction::LeaveLobby);
        }
    });
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

pub fn draw_queue_overlay(
    ui: &mut Ui,
    state: &MainMenuState,
    action_min_h: f32,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
) {
    let metrics = super::layout::main_menu_metrics(ui.ctx());
    let compact = metrics.is_compact();
    let scale = metrics.scale;
    let lobby_info = resolve_lobby_info(state);
    let panel_frame = sow_ui_kit::theme::standard_panel_frame(compact);
    let available_rect = ui.available_rect_before_wrap();

    // Compact: the card is the whole viewport (minus the footer panel, which the
    // caller already excludes). Desktop: centered card with scale-aware margins.
    let card_rect = if compact {
        available_rect
    } else {
        let pad_x = (24.0 * scale).clamp(8.0, 32.0);
        let pad_y = (32.0 * scale).clamp(6.0, 48.0);
        let card_w = (available_rect.width() - pad_x * 2.0).max(320.0);
        let card_h = (available_rect.height() - pad_y * 2.0).max(220.0);
        let x = available_rect.min.x + (available_rect.width() - card_w) * 0.5;
        let y = available_rect.min.y + (available_rect.height() - card_h) * 0.5;
        egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(card_w, card_h))
    };

    ui.scope_builder(egui::UiBuilder::new().max_rect(card_rect), |ui| {
        // Hard boundary: nothing inside the card may paint past its edge.
        ui.set_clip_rect(ui.clip_rect().intersect(card_rect));
        panel_frame.show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            let full = ui.available_rect_before_wrap();
            let bottom_inset = if compact { state.safe_area_bottom } else { 0.0 };
            let inner = egui::Rect::from_min_max(
                full.min,
                egui::pos2(
                    full.max.x,
                    (full.max.y - bottom_inset).max(full.min.y + 120.0),
                ),
            );

            ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                ui.set_clip_rect(ui.clip_rect().intersect(inner));

                let Some(lobby) = lobby_info else {
                    draw_lobby_connecting(ui, inner.height(), lang);
                    return;
                };

                if lobby_countdown_active(state, lobby) {
                    draw_countdown_strip(ui, &countdown_label(state, lobby), compact);
                }

                let body = ui.available_rect_before_wrap();
                let host = HostControls::from_state(state, lobby.id);
                let body_opts = QueueBodyOpts {
                    state,
                    lobby,
                    asset_loader,
                    lang,
                    body,
                    host,
                    action_min_h,
                    action,
                };

                if compact {
                    draw_body_compact(ui, body_opts);
                } else {
                    draw_body_desktop(ui, body_opts);
                }
            });
        });
    });
}

struct QueueBodyOpts<'a> {
    state: &'a MainMenuState,
    lobby: &'a sow_core::protocol::LobbyInfo,
    asset_loader: &'a crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    body: egui::Rect,
    host: HostControls,
    action_min_h: f32,
    action: &'a mut Option<UiAction>,
}

/// Desktop: two equal columns filling `body`. Left = map summary with the
/// action row pinned to its bottom edge; right = players roster.
fn draw_body_desktop(ui: &mut Ui, opts: QueueBodyOpts<'_>) {
    let state = opts.state;
    let lobby = opts.lobby;
    let asset_loader = opts.asset_loader;
    let lang = opts.lang;
    let body = opts.body;
    let host = opts.host;
    let action_min_h = opts.action_min_h;
    let action = opts.action;
    let col_w = (body.width() - COLUMN_GAP) * 0.5;
    let left = egui::Rect::from_min_size(body.min, egui::vec2(col_w, body.height()));
    let right = egui::Rect::from_min_size(
        egui::pos2(left.max.x + COLUMN_GAP, body.min.y),
        egui::vec2(col_w, body.height()),
    );

    ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| {
        draw_summary_column(ui, state, lobby, asset_loader, lang, action_min_h, action);
    });
    ui.scope_builder(egui::UiBuilder::new().max_rect(right), |ui| {
        draw_players_panel(ui, lobby, asset_loader, lang, host, action, true);
    });
}

/// Compact / mobile: one scrollable column (summary + players) with the action
/// row pinned to the bottom of the card, always visible.
fn draw_body_compact(ui: &mut Ui, opts: QueueBodyOpts<'_>) {
    let state = opts.state;
    let lobby = opts.lobby;
    let asset_loader = opts.asset_loader;
    let lang = opts.lang;
    let body = opts.body;
    let host = opts.host;
    let action_min_h = opts.action_min_h;
    let action = opts.action;
    // Total vertical space owned by the pinned action row, including the padding
    // that keeps it clear of the card's bottom edge.
    let actions_zone_h = action_min_h + ACTIONS_BOTTOM_PAD;
    let content_h = (body.height() - actions_zone_h - ACTIONS_TOP_GAP).max(120.0);
    let content_rect = egui::Rect::from_min_size(body.min, egui::vec2(body.width(), content_h));
    // Buttons sit above the bottom pad; the row is exactly `action_min_h` tall.
    let actions_rect = egui::Rect::from_min_max(
        egui::pos2(body.min.x, body.max.y - actions_zone_h),
        egui::pos2(body.max.x, body.max.y - ACTIONS_BOTTOM_PAD),
    );

    ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
        ui.set_clip_rect(ui.clip_rect().intersect(content_rect));
        egui::ScrollArea::vertical()
            .id_salt("lobby_body_compact")
            .max_height(content_rect.height())
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                summary_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    draw_summary_content(ui, lobby, asset_loader, lang, None);
                });
                ui.add_space(8.0);
                draw_players_panel(ui, lobby, asset_loader, lang, host, action, false);
            });
    });

    // The action row must never clip, so it is NOT clipped to its own rect — it
    // gets a guaranteed slot the content region already stayed clear of.
    ui.scope_builder(egui::UiBuilder::new().max_rect(actions_rect), |ui| {
        draw_lobby_actions(ui, state, lobby, action_min_h, action, lang);
    });
}

fn summary_frame() -> Frame {
    Frame::NONE
        .fill(sow_ui_kit::theme::palette::field_bg())
        .stroke(Stroke::new(
            1.0_f32,
            sow_ui_kit::theme::palette::field_border(),
        ))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(16.0)
}

/// Left desktop column: framed summary with the action row pinned to the frame
/// bottom. Content is clipped to its own region so the buttons stay visible no
/// matter how much data the lobby carries.
fn draw_summary_column(
    ui: &mut Ui,
    state: &MainMenuState,
    lobby: &sow_core::protocol::LobbyInfo,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    action_min_h: f32,
    action: &mut Option<UiAction>,
) {
    summary_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_height(ui.available_height());

        let inner = ui.available_rect_before_wrap();
        let content_h = (inner.height() - action_min_h - ACTIONS_TOP_GAP).max(60.0);
        let content_rect =
            egui::Rect::from_min_size(inner.min, egui::vec2(inner.width(), content_h));
        let actions_rect = egui::Rect::from_min_max(
            egui::pos2(inner.min.x, inner.max.y - action_min_h),
            inner.max,
        );

        ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(content_rect));
            draw_summary_content(ui, lobby, asset_loader, lang, Some(content_h));
        });
        ui.scope_builder(egui::UiBuilder::new().max_rect(actions_rect), |ui| {
            draw_lobby_actions(ui, state, lobby, action_min_h, action, lang);
        });
    });
}

/// Map thumbnail on top, then one row: map name + mode chip on the left,
/// lobby details (room code, host, bots, …) on the right.
fn draw_summary_content(
    ui: &mut Ui,
    lobby: &sow_core::protocol::LobbyInfo,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    height_budget: Option<f32>,
) {
    let strings = &sow_i18n::get(lang).main_menu;

    let mut details: Vec<(&str, String)> = Vec::new();
    if is_custom_lobby(lobby) {
        details.push((&strings.lobby_code_label, lobby.id.to_string()));
    }
    if !lobby.host_name.is_empty() {
        details.push((&strings.host_label, lobby.host_name.clone()));
    }
    if lobby.bot_count > 0 {
        details.push((&strings.bots_label, lobby.bot_count.to_string()));
        details.push((
            &strings.bot_difficulty,
            format!("{:?}", lobby.bot_difficulty),
        ));
    }
    if lobby.nation_count > 0 {
        details.push((&strings.nations_count, lobby.nation_count.to_string()));
    }
    if lobby.has_password {
        details.push((&strings.password_label, "LOCKED".to_string()));
    }

    let details_h = details.len() as f32 * 20.0;
    let info_block_h = details_h.max(64.0);

    let thumbnail = asset_loader.thumbnail(&lobby.map_name);
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
    let max_img_h = match height_budget {
        Some(budget) => (budget - info_block_h - 10.0).max(50.0),
        None => 160.0,
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
            egui::CornerRadius::same(8),
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

    ui.add_space(10.0);

    // Info row: title + mode chip left, detail key/value list right.
    ui.horizontal_top(|ui| {
        let total_w = ui.available_width();
        let details_w = (total_w * 0.45).clamp(140.0, 220.0);
        let title_w = (total_w - details_w - 8.0).max(100.0);

        ui.allocate_ui_with_layout(
            egui::vec2(title_w, info_block_h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.label(
                    RichText::new(lobby.map_name.to_uppercase())
                        .size(22.0)
                        .strong()
                        .color(Color32::WHITE),
                );
                ui.add_space(4.0);

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
                } else if lobby.game_mode == "HumansVsNations" {
                    (&strings.humans_vs_nations, Color32::from_rgb(74, 222, 128))
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
            },
        );

        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for (key, val) in &details {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(*key)
                            .size(12.0)
                            .color(sow_ui_kit::theme::palette::text_muted()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        crate::widgets::outlined_emoji_label(
                            ui,
                            val,
                            egui::FontId::proportional(12.0),
                            Color32::WHITE,
                        );
                    });
                });
            }
        });
    });
}

fn draw_players_panel(
    ui: &mut Ui,
    lobby: &sow_core::protocol::LobbyInfo,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    host: HostControls,
    action: &mut Option<UiAction>,
    fill_height: bool,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    let is_teams = lobby.game_mode == "Teams";
    // Team chips render for any team-based mode; the host "MOVE TEAM" toggle
    // stays Teams-only (HumansVsNations forces every human to Red).
    let show_team_chips = is_teams || lobby.game_mode == "HumansVsNations";

    let menu_id = egui::Id::new("lobby_roster_action_menu");
    let menu_open: Option<u16> = ui
        .ctx()
        .data(|d| d.get_temp::<Option<u16>>(menu_id))
        .flatten();
    let mut new_menu_open = menu_open;
    let mut name_click: Option<u16> = None;
    let mut click_elsewhere = false;

    summary_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        if fill_height {
            ui.set_height(ui.available_height());
        }
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&strings.players_label)
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
            ui.add_space(10.0);

            let remaining_h = ui.available_height().max(120.0);
            egui::ScrollArea::vertical()
                .max_height(remaining_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 6.0);

                    if is_teams {
                        let (red_players, blue_players): (Vec<_>, Vec<_>) = lobby
                            .players
                            .iter()
                            .partition(|p| p.team == Some(sow_core::protocol::Team::Red));

                        if fill_height {
                            // Desktop Teams mode: 2 columns side-by-side (Red Left, Blue Right)
                            ui.columns(2, |cols| {
                                cols[0].vertical(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 6.0);
                                    team_section_header(
                                        ui,
                                        "RED TEAM",
                                        red_players.len(),
                                        Color32::from_rgb(239, 68, 68),
                                    );
                                    for &p in &red_players {
                                        draw_player_row(
                                            ui,
                                            p,
                                            asset_loader,
                                            strings,
                                            host,
                                            false,
                                            is_teams,
                                            action,
                                            &mut name_click,
                                            menu_open,
                                            &mut new_menu_open,
                                            &mut click_elsewhere,
                                        );
                                    }
                                });
                                cols[1].vertical(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 6.0);
                                    team_section_header(
                                        ui,
                                        "BLUE TEAM",
                                        blue_players.len(),
                                        Color32::from_rgb(59, 130, 246),
                                    );
                                    for &p in &blue_players {
                                        draw_player_row(
                                            ui,
                                            p,
                                            asset_loader,
                                            strings,
                                            host,
                                            false,
                                            is_teams,
                                            action,
                                            &mut name_click,
                                            menu_open,
                                            &mut new_menu_open,
                                            &mut click_elsewhere,
                                        );
                                    }
                                });
                            });
                        } else {
                            // Mobile Teams mode: single column grouped by team
                            team_section_header(
                                ui,
                                "RED TEAM",
                                red_players.len(),
                                Color32::from_rgb(239, 68, 68),
                            );
                            for &p in &red_players {
                                draw_player_row(
                                    ui,
                                    p,
                                    asset_loader,
                                    strings,
                                    host,
                                    show_team_chips,
                                    is_teams,
                                    action,
                                    &mut name_click,
                                    menu_open,
                                    &mut new_menu_open,
                                    &mut click_elsewhere,
                                );
                            }
                            ui.add_space(8.0);
                            team_section_header(
                                ui,
                                "BLUE TEAM",
                                blue_players.len(),
                                Color32::from_rgb(59, 130, 246),
                            );
                            for &p in &blue_players {
                                draw_player_row(
                                    ui,
                                    p,
                                    asset_loader,
                                    strings,
                                    host,
                                    show_team_chips,
                                    is_teams,
                                    action,
                                    &mut name_click,
                                    menu_open,
                                    &mut new_menu_open,
                                    &mut click_elsewhere,
                                );
                            }
                        }
                    } else if fill_height && lobby.players.len() > 6 {
                        // Desktop FFA/HvN long list: 2 balanced columns
                        let mid = (lobby.players.len() + 1) / 2;
                        let (left_players, right_players) = lobby.players.split_at(mid);
                        ui.columns(2, |cols| {
                            cols[0].vertical(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(0.0, 6.0);
                                for p in left_players {
                                    draw_player_row(
                                        ui,
                                        p,
                                        asset_loader,
                                        strings,
                                        host,
                                        show_team_chips,
                                        is_teams,
                                        action,
                                        &mut name_click,
                                        menu_open,
                                        &mut new_menu_open,
                                        &mut click_elsewhere,
                                    );
                                }
                            });
                            cols[1].vertical(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(0.0, 6.0);
                                for p in right_players {
                                    draw_player_row(
                                        ui,
                                        p,
                                        asset_loader,
                                        strings,
                                        host,
                                        show_team_chips,
                                        is_teams,
                                        action,
                                        &mut name_click,
                                        menu_open,
                                        &mut new_menu_open,
                                        &mut click_elsewhere,
                                    );
                                }
                            });
                        });
                    } else {
                        // Standard single column
                        for p in &lobby.players {
                            draw_player_row(
                                ui,
                                p,
                                asset_loader,
                                strings,
                                host,
                                show_team_chips,
                                is_teams,
                                action,
                                &mut name_click,
                                menu_open,
                                &mut new_menu_open,
                                &mut click_elsewhere,
                            );
                        }
                    }
                });
        });
    });

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

fn team_section_header(ui: &mut Ui, label: &str, count: usize, color: Color32) {
    ui.horizontal(|ui| {
        Frame::NONE
            .fill(color.linear_multiply(0.18))
            .stroke(Stroke::new(1.0_f32, color))
            .corner_radius(CornerRadius::same(4))
            .inner_margin(Margin::symmetric(8, 3))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("{label} ({count})"))
                        .size(11.0)
                        .strong()
                        .color(color),
                );
            });
    });
    ui.add_space(2.0);
}

fn draw_player_row(
    ui: &mut Ui,
    p: &sow_core::protocol::LobbyPlayerSyncState,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    strings: &sow_i18n::MainMenuStrings,
    host: HostControls,
    show_team_chips: bool,
    is_teams: bool,
    action: &mut Option<UiAction>,
    name_click: &mut Option<u16>,
    menu_open: Option<u16>,
    new_menu_open: &mut Option<u16>,
    click_elsewhere: &mut bool,
) {
    let can_moderate = host.is_host && Some(p.player_id) != host.my_player_id;

    let row = Frame::NONE
        .fill(sow_ui_kit::theme::palette::surface_transparent())
        .stroke(Stroke::new(
            1.0_f32,
            sow_ui_kit::theme::palette::field_border(),
        ))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 8))
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
                            .fit_to_exact_size(egui::vec2(24.0, 24.0))
                            .corner_radius(CornerRadius::same(12)),
                    );
                }

                ui.add_space(6.0);

                let sense = if can_moderate {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                };
                let mut name_resp = ui.add(
                    egui::Label::new(
                        RichText::new(&p.name)
                            .size(14.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .sense(sense),
                );
                if can_moderate {
                    name_resp = name_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    if name_resp.clicked() {
                        *name_click = Some(p.player_id);
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let map_ready = p.download_progress == 100 || p.is_ready;
                    if map_ready {
                        status_badge(ui, &strings.ready, Color32::from_rgb(74, 222, 128));
                    } else {
                        status_badge(
                            ui,
                            &format!("SYNCING {}%", p.download_progress),
                            Color32::from_rgb(250, 204, 21),
                        );
                    }
                    if show_team_chips {
                        if let Some((label, col)) = team_chip(p.team) {
                            ui.add_space(4.0);
                            status_badge(ui, label, col);
                        }
                    }
                });

                name_resp.rect
            })
            .inner
        });

    if menu_open == Some(p.player_id) && can_moderate {
        let area = egui::Area::new(egui::Id::new(("lobby_roster_menu", p.player_id)))
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
                                crate::widgets::ThemeButton::new(&strings.kick_btn)
                                    .style(crate::widgets::ThemeButtonStyle::Secondary)
                                    .text_size(12.0)
                                    .min_size(egui::vec2(bw, 44.0)),
                            )
                            .clicked()
                        {
                            *action = Some(UiAction::KickPlayer {
                                lobby_id: host.lobby_id,
                                target_player_id: p.player_id,
                            });
                            *new_menu_open = None;
                        }
                        if is_teams
                            && ui
                                .add(
                                    crate::widgets::ThemeButton::new(&strings.move_team_btn)
                                        .style(crate::widgets::ThemeButtonStyle::Secondary)
                                        .text_size(12.0)
                                        .min_size(egui::vec2(bw, 44.0)),
                                )
                                .clicked()
                        {
                            *action = Some(UiAction::MovePlayerTeam {
                                lobby_id: host.lobby_id,
                                target_player_id: p.player_id,
                            });
                            *new_menu_open = None;
                        }
                        if ui
                            .add(
                                crate::widgets::ThemeButton::new(&strings.ban_btn)
                                    .style(crate::widgets::ThemeButtonStyle::Danger)
                                    .text_size(12.0)
                                    .min_size(egui::vec2(bw, 44.0)),
                            )
                            .clicked()
                        {
                            *action = Some(UiAction::BanPlayer {
                                lobby_id: host.lobby_id,
                                target_player_id: p.player_id,
                            });
                            *new_menu_open = None;
                        }
                    });
            });
        if area.response.clicked_elsewhere() {
            *click_elsewhere = true;
        }
    }
}

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

fn team_chip(team: Option<sow_core::protocol::Team>) -> Option<(&'static str, Color32)> {
    match team {
        Some(sow_core::protocol::Team::Red) => Some(("RED", Color32::from_rgb(239, 68, 68))),
        Some(sow_core::protocol::Team::Blue) => Some(("BLUE", Color32::from_rgb(59, 130, 246))),
        None => None,
    }
}
