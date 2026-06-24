pub mod actions;
pub mod browser;
pub mod create_game;
pub mod join_browser;
mod layout;
mod modals;
pub mod profile;
pub mod queue_overlay;
pub mod single_player_setup;

use crate::UiAction;
use egui::{Align, CentralPanel, Color32, Frame, Layout};
use sow_core::protocol::LobbyInfo;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GameModeFilter {
    #[default]
    All,
    Ffa,
    Teams,
}

/// Brief auto-dismissing notice shown after the server pulls a player out of a lobby.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyNotice {
    /// The host abandoned the lobby; everyone was returned to the menu.
    HostLeft,
    /// The host kicked this player (they may rejoin).
    Kicked,
    /// The host banned this player from that lobby.
    Banned,
}

#[derive(Clone, Debug)]
pub struct UiLinkConflictInfo {
    pub current_account_id: String,
    pub existing_account_id: String,
    pub current_level: u32,
    pub existing_level: u32,
    pub target_provider: String,
    pub target_external_id: String,
}

pub struct MainMenuState {
    pub is_connected: bool,
    pub is_connecting: bool,
    pub is_waiting: bool,
    pub wait_timer_secs: f32,
    pub server_address: String,
    pub lobbies: Vec<LobbyInfo>,
    pub player_name: String,
    /// Portal SDK locked the display name (CrazyGames username, etc.).
    pub name_locked: bool,
    pub host_private_pending: bool,
    pub in_private_match: bool,
    pub is_lobby_host: bool,
    // Create Game screen
    pub show_create_game: bool,
    pub create_game_config: Box<sow_core::game_config::GameConfig>,
    pub create_game_is_private: bool,
    pub create_game_password: String,
    // Join Browser screen
    pub show_join_browser: bool,
    pub join_mode_filter: GameModeFilter,
    pub join_lobby_code: String,
    pub join_password_input: String,
    pub join_password_for_lobby: Option<u64>,
    pub pending_join_lobby_id: Option<u64>,
    pub joined_lobby_id: Option<u64>,
    pub downloading_map_name: Option<String>,
    pub is_downloading_map: bool,
    pub cached_map: Option<Vec<u8>>,
    /// Folder key of the map whose terrain bytes are cached for offline start.
    pub cached_map_key: Option<String>,
    pub map_download_progress: u8,
    pub show_leader_picker: bool,
    pub clan_tag: String,
    pub selected_leader: sow_core::player::Leader,
    pub selected_civilization: sow_core::player::Civilization,
    pub show_single_player_setup: bool,
    pub single_player_config: Box<sow_core::game_config::GameConfig>,
    pub error_message: Option<String>,
    pub leader_backdrop: crate::widgets::LeaderBackdropTransition,
    pub invite_copied_at: Option<f64>,
    pub active_conflict: Option<UiLinkConflictInfo>,
    /// Local player's id in the joined lobby — lets the host roster skip its own entry.
    pub my_player_id: Option<u16>,
    /// Active lobby notice (host left / kicked / banned) and the frame time it appeared.
    pub notice: Option<LobbyNotice>,
    pub notice_at: Option<f64>,
}

impl Default for MainMenuState {
    fn default() -> Self {
        let ms = web_time::SystemTime::now()
            .duration_since(web_time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let leader = match ms % 12 {
            0 => sow_core::player::Leader::Caesar,
            1 => sow_core::player::Leader::Cleopatra,
            2 => sow_core::player::Leader::Ragnar,
            3 => sow_core::player::Leader::SunTzu,
            4 => sow_core::player::Leader::Alexander,
            5 => sow_core::player::Leader::GenghisKhan,
            6 => sow_core::player::Leader::RichardTheLionheart,
            7 => sow_core::player::Leader::Vercingetorix,
            8 => sow_core::player::Leader::Boudica,
            9 => sow_core::player::Leader::LadySixSky,
            10 => sow_core::player::Leader::Leonidas,
            _ => sow_core::player::Leader::Napoleon,
        };
        let civ = crate::widgets::avatar_picker::leader_civilization(leader);
        Self {
            is_connected: false,
            is_connecting: false,
            is_waiting: false,
            wait_timer_secs: 0.0,
            server_address: std::env::var("SOW_WS_URL")
                .unwrap_or_else(|_| "wss://shadowsofwar.io/ws/".to_string()),
            lobbies: Vec::new(),
            player_name: format!("ANON{:03}", ms % 1000),
            clan_tag: "".to_string(),
            selected_leader: leader,
            selected_civilization: civ,
            name_locked: false,
            host_private_pending: false,
            in_private_match: false,
            is_lobby_host: false,
            show_create_game: false,
            create_game_config: Box::new(sow_core::game_config::GameConfig {
                seed: ms as u64,
                ..Default::default()
            }),
            create_game_is_private: false,
            create_game_password: String::new(),
            show_join_browser: false,
            join_mode_filter: GameModeFilter::All,
            join_lobby_code: String::new(),
            join_password_input: String::new(),
            join_password_for_lobby: None,
            pending_join_lobby_id: None,
            joined_lobby_id: None,
            downloading_map_name: None,
            is_downloading_map: false,
            cached_map: None,
            cached_map_key: None,
            map_download_progress: 0,
            show_leader_picker: false,
            show_single_player_setup: false,
            single_player_config: Box::new(sow_core::game_config::GameConfig {
                seed: ms as u64,
                ..Default::default()
            }),
            active_conflict: None,
            leader_backdrop: crate::widgets::LeaderBackdropTransition::new(leader),
            error_message: None,
            invite_copied_at: None,
            my_player_id: None,
            notice: None,
            notice_at: None,
        }
    }
}

impl MainMenuState {
    /// Clamp skirmish map selection to a valid catalog entry (dimensions included).
    pub fn apply_map_catalog(&mut self, catalog: &[sow_core::maps::MapCatalogEntry]) {
        let cfg = &mut self.single_player_config;
        cfg.map_name = sow_core::maps::resolve_map_name(catalog, &cfg.map_name);
        sow_core::maps::apply_catalog_dimensions(
            catalog,
            &mut cfg.map_name,
            &mut cfg.map_width,
            &mut cfg.map_height,
        );
    }

    pub fn apply_map_catalog_create(&mut self, catalog: &[sow_core::maps::MapCatalogEntry]) {
        let cfg = &mut self.create_game_config;
        cfg.map_name = sow_core::maps::resolve_map_name(catalog, &cfg.map_name);
        sow_core::maps::apply_catalog_dimensions(
            catalog,
            &mut cfg.map_name,
            &mut cfg.map_width,
            &mut cfg.map_height,
        );
    }
}

pub fn primary_lobby_for_browser(lobbies: &[LobbyInfo]) -> Option<LobbyInfo> {
    if lobbies.is_empty() {
        return None;
    }
    let mut counting: Vec<&LobbyInfo> = lobbies.iter().filter(|l| l.is_counting_down).collect();
    if !counting.is_empty() {
        counting.sort_by_key(|l| l.id);
        return Some(counting[0].clone());
    }
    let mut rest: Vec<&LobbyInfo> = lobbies.iter().collect();
    rest.sort_by_key(|l| l.id);
    Some(rest[0].clone())
}

pub fn draw_terms_privacy_footer(
    ui: &mut egui::Ui,
    lang: sow_i18n::Language,
    action: &mut Option<UiAction>,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    let version = format!("v{}", include_str!("../../../../.version").trim());
    let credits = &sow_i18n::get(lang).credits;
    let text_color = sow_ui_kit::theme::palette::text_muted();
    let link_color = sow_ui_kit::theme::palette::neon_cyan();
    let size = 11.0;
    let narrow = ui.available_width() < 480.0;

    let draw_terms_link = |ui: &mut egui::Ui, action: &mut Option<UiAction>| {
        let terms_id = ui.make_persistent_id("terms_of_service_link");
        let terms_hover_t = ui.ctx().animate_bool(terms_id, false);
        let mut terms_text = egui::RichText::new(&sow_i18n::get(lang).settings.terms_of_service)
            .font(sow_ui_kit::theme::font_regular(size))
            .color(link_color);
        if terms_hover_t > 0.05 {
            terms_text = terms_text.underline();
        }
        let terms_resp = ui.add(egui::Label::new(terms_text).sense(egui::Sense::click()));
        ui.ctx().animate_bool(terms_id, terms_resp.hovered());
        if terms_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if terms_resp.clicked() {
            *action = Some(UiAction::ToggleTerms);
        }
    };

    let draw_privacy_link = |ui: &mut egui::Ui, action: &mut Option<UiAction>| {
        let privacy_id = ui.make_persistent_id("privacy_policy_link");
        let privacy_hover_t = ui.ctx().animate_bool(privacy_id, false);
        let mut privacy_text = egui::RichText::new(&sow_i18n::get(lang).settings.privacy_policy)
            .font(sow_ui_kit::theme::font_regular(size))
            .color(link_color);
        if privacy_hover_t > 0.05 {
            privacy_text = privacy_text.underline();
        }
        let privacy_resp = ui.add(egui::Label::new(privacy_text).sense(egui::Sense::click()));
        ui.ctx().animate_bool(privacy_id, privacy_resp.hovered());
        if privacy_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if privacy_resp.clicked() {
            *action = Some(UiAction::TogglePrivacy);
        }
    };

    let draw_discord_link = |ui: &mut egui::Ui| {
        let text = egui::RichText::new("Discord")
            .font(sow_ui_kit::theme::font_regular(size))
            .color(link_color);
        ui.hyperlink_to(text, "https://discord.gg/eauHRf7zP");
    };

    let draw_github_link = |ui: &mut egui::Ui| {
        let text = egui::RichText::new("GitHub")
            .font(sow_ui_kit::theme::font_regular(size))
            .color(link_color);
        ui.hyperlink_to(text, "https://github.com/ohsalmeron/shadows-of-war");
    };

    if narrow {
        ui.with_layout(
            egui::Layout::top_down(egui::Align::Center).with_cross_align(egui::Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(
                        egui::RichText::new(strings.by_playing_you_agree.trim_end())
                            .font(sow_ui_kit::theme::font_regular(size))
                            .color(text_color),
                    );
                    draw_terms_link(ui, action);
                    ui.label(
                        egui::RichText::new(strings.and_the.trim())
                            .font(sow_ui_kit::theme::font_regular(size))
                            .color(text_color),
                    );
                    draw_privacy_link(ui, action);
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(
                        egui::RichText::new(&version)
                            .font(sow_ui_kit::theme::font_regular(size))
                            .color(text_color),
                    );
                    ui.label(
                        egui::RichText::new("·")
                            .font(sow_ui_kit::theme::font_regular(size))
                            .color(text_color),
                    );
                    ui.label(
                        egui::RichText::new(&credits.based_on_short)
                            .font(sow_ui_kit::theme::font_regular(size))
                            .color(text_color),
                    );
                    ui.label(
                        egui::RichText::new("·")
                            .font(sow_ui_kit::theme::font_regular(size))
                            .color(text_color),
                    );
                    draw_discord_link(ui);
                    ui.label(
                        egui::RichText::new("·")
                            .font(sow_ui_kit::theme::font_regular(size))
                            .color(text_color),
                    );
                    draw_github_link(ui);
                });
            },
        );
        return;
    }

    ui.with_layout(
        egui::Layout::left_to_right(egui::Align::Center).with_main_align(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            ui.label(
                egui::RichText::new(strings.by_playing_you_agree.trim_end())
                    .font(sow_ui_kit::theme::font_regular(size))
                    .color(text_color),
            );

            draw_terms_link(ui, action);

            ui.label(
                egui::RichText::new(strings.and_the.trim())
                    .font(sow_ui_kit::theme::font_regular(size))
                    .color(text_color),
            );

            draw_privacy_link(ui, action);

            ui.label(
                egui::RichText::new("·")
                    .font(sow_ui_kit::theme::font_regular(size))
                    .color(text_color),
            );

            ui.label(
                egui::RichText::new(&version)
                    .font(sow_ui_kit::theme::font_regular(size))
                    .color(text_color),
            );

            ui.label(
                egui::RichText::new("·")
                    .font(sow_ui_kit::theme::font_regular(size))
                    .color(text_color),
            );

            ui.label(
                egui::RichText::new(&credits.based_on_short)
                    .font(sow_ui_kit::theme::font_regular(size))
                    .color(text_color),
            );

            ui.label(
                egui::RichText::new("·")
                    .font(sow_ui_kit::theme::font_regular(size))
                    .color(text_color),
            );

            draw_discord_link(ui);

            ui.label(
                egui::RichText::new("·")
                    .font(sow_ui_kit::theme::font_regular(size))
                    .color(text_color),
            );

            draw_github_link(ui);
        },
    );
}

pub fn draw(
    root_ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    reduced_motion: bool,
) -> Option<UiAction> {
    let mut action = None;
    let compact = sow_ui_kit::theme::compact_viewport(root_ui.ctx());
    let outer_pad = 16.0;
    let strings = &sow_i18n::get(lang).main_menu;

    // Draw the full-bleed backdrop first so that all panels (including the footer)
    // are drawn on top of it.
    if !state.show_leader_picker && !(state.show_join_browser && !state.is_waiting) {
        let backdrop_rect = root_ui.ctx().content_rect();
        let use_portrait = backdrop_rect.width() < backdrop_rect.height();
        crate::widgets::draw_leader_hero_backdrop(
            root_ui,
            backdrop_rect,
            state.selected_leader,
            use_portrait,
            asset_loader,
            &mut state.leader_backdrop,
            &strings.loading_leader_portrait,
            false,
        );
    }

    egui::Panel::bottom("main_menu_footer_panel")
        .frame(
            egui::Frame::NONE
                .fill(sow_ui_kit::theme::palette::surface())
                .inner_margin(egui::Margin::symmetric(16, 8)),
        )
        .show_inside(root_ui, |ui| {
            draw_terms_privacy_footer(ui, lang, &mut action);
        });

    if state.show_join_browser && !state.is_waiting {
        join_browser::draw(root_ui, state, asset_loader, &mut action, lang);
    } else {
        CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(Color32::TRANSPARENT)
                    .inner_margin(outer_pad),
            )
            .show_inside(root_ui, |ui| {
                if state.is_waiting {
                    // Always fullscreen — the lobby waiting room fills the viewport at
                    // every resolution (no cramped centered modal).
                    let (section_gap, action_min_h, _, _) = layout::menu_layout_chrome(
                        ui.ctx(),
                        ui.available_height(),
                        ui.available_width(),
                        compact,
                    );
                    queue_overlay::draw_queue_overlay(
                        ui,
                        state,
                        section_gap,
                        action_min_h,
                        &mut action,
                        asset_loader,
                        lang,
                    );
                } else {
                    let panel_w = sow_ui_kit::theme::menu_rail_panel_width(
                        ui.available_width(),
                        compact,
                        ui.ctx(),
                    );
                    let inner_w = if compact {
                        panel_w
                    } else {
                        (panel_w - 40.0).max(0.0)
                    };
                    let content_h = ui.available_height();
                    let effective_h = if compact {
                        content_h
                    } else {
                        (content_h - 40.0).max(0.0)
                    };
                    let (section_gap, action_min_h, profile_height, lobby_height) =
                        layout::menu_layout_chrome(ui.ctx(), effective_h, inner_w, compact);

                    ui.allocate_ui_with_layout(
                        egui::vec2(panel_w, content_h),
                        Layout::top_down(Align::Min),
                        |ui| {
                            layout::draw_menu_right_panel(
                                ui,
                                state,
                                section_gap,
                                action_min_h,
                                compact,
                                profile_height,
                                lobby_height,
                                content_h,
                                &mut action,
                                asset_loader,
                                lang,
                            );
                        },
                    );
                }
            });
    }

    if state.show_create_game {
        create_game::draw(
            root_ui,
            state,
            asset_loader,
            &mut action,
            lang,
            reduced_motion,
        );
    }
    if state.show_single_player_setup {
        single_player_setup::draw(
            root_ui,
            state,
            asset_loader,
            &mut action,
            lang,
            reduced_motion,
        );
    }

    if let Some(conflict) = state.active_conflict.clone() {
        modals::draw_link_conflict_modal(root_ui, &mut action, &conflict, lang, compact);
    }

    modals::draw_connecting_indicator(root_ui.ctx(), state, lang, compact);
    modals::draw_map_download_indicator(root_ui.ctx(), state, lang, compact);

    if state.show_leader_picker
        && crate::widgets::draw_leader_picker_modal(
            root_ui.ctx(),
            &mut state.selected_leader,
            &mut state.selected_civilization,
            asset_loader,
            &mut state.leader_backdrop,
            lang,
        )
    {
        state.show_leader_picker = false;
    }

    if let Some(err_msg) = state.error_message.clone() {
        modals::draw_connection_error_modal(
            root_ui,
            state,
            &mut action,
            &err_msg,
            strings,
            compact,
        );
    }

    if let Some(notice) = state.notice {
        let now = root_ui.input(|i| i.time);
        let shown_at = *state.notice_at.get_or_insert(now);
        let dismissed = modals::draw_lobby_notice(root_ui, notice, strings, compact);
        // Keep repainting so the 3s auto-dismiss fires even without further input.
        root_ui.ctx().request_repaint();
        if dismissed || now - shown_at >= 3.0 {
            state.notice = None;
            state.notice_at = None;
        }
    }

    action
}
