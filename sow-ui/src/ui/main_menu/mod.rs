pub mod browser;
pub mod custom_game;
pub mod join_browser;
mod layout;
mod modals;
pub mod profile;
pub mod queue_overlay;
pub mod store;

use crate::UiAction;
use sow_core::protocol::LobbyInfo;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GameModeFilter {
    #[default]
    All,
    Ffa,
    Teams,
    HumansVsNations,
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
    /// Connection to server or relay was lost / unavailable.
    ConnectionLost,
}

/// The one source of truth for the native menu screen. Queue is represented by
/// the network-owned `is_waiting` flag while a match is being joined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainMenuRoute {
    Home,
    Browser,
    Create,
    Queue,
    Store,
    Profile,
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
    pub custom_game_is_private: bool,
    // Custom Game screen (unified single-player + create)
    pub custom_game_is_sp: bool,
    pub custom_game_config: Box<sow_core::game_config::GameConfig>,
    pub custom_game_password: String,
    // Join Browser screen
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
    pub error_message: Option<String>,
    pub leader_backdrop: crate::widgets::LeaderBackdropTransition,
    /// Local player's id in the joined lobby — lets the host roster skip its own entry.
    pub my_player_id: Option<u16>,
    /// Active lobby notice (host left / kicked / banned) and the frame time it appeared.
    pub notice: Option<LobbyNotice>,
    pub notice_at: Option<f64>,
    pub safe_area_bottom: f32,
    /// Compact account progression shown beside the identity header.
    pub account_level: u32,
    pub account_xp: u32,
    pub laurels: u64,
    /// Authoritative commerce snapshot copied from the client profile.
    pub store_catalog: sow_data::commerce::StoreCatalog,
    pub selected_skin: Option<String>,
    pub store_busy: bool,
    pub profile: profile::NativeProfileState,
    pub route: MainMenuRoute,
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
        let empty_leaders = BTreeSet::new();
        let empty_skins = BTreeSet::new();
        Self {
            is_connected: false,
            is_connecting: false,
            is_waiting: false,
            wait_timer_secs: 0.0,
            server_address: std::env::var("SOW_WS_URL")
                .unwrap_or_else(|_| "wss://ws.shadowsofwar.io/ws/".to_string()),
            lobbies: Vec::new(),
            player_name: format!("ANON{:03}", ms % 1000),
            clan_tag: "".to_string(),
            selected_leader: leader,
            selected_civilization: civ,
            name_locked: false,
            host_private_pending: false,
            in_private_match: false,
            is_lobby_host: false,
            custom_game_is_sp: true,
            custom_game_config: Box::new(sow_core::game_config::GameConfig {
                seed: ms as u64,
                ..Default::default()
            }),
            custom_game_password: String::new(),
            custom_game_is_private: false,
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
            leader_backdrop: crate::widgets::LeaderBackdropTransition::new(leader),
            error_message: None,
            my_player_id: None,
            notice: None,
            notice_at: None,
            safe_area_bottom: 0.0,
            account_level: 1,
            account_xp: 0,
            laurels: 0,
            store_catalog: sow_data::commerce::catalog_for_profile(
                &empty_leaders,
                &empty_skins,
                0,
                0,
                0,
            ),
            selected_skin: None,
            store_busy: false,
            profile: profile::NativeProfileState::default(),
            route: MainMenuRoute::Home,
        }
    }
}

impl MainMenuState {
    pub fn visible_route(&self) -> MainMenuRoute {
        if self.is_waiting {
            MainMenuRoute::Queue
        } else {
            self.route
        }
    }

    pub fn open_route(&mut self, route: MainMenuRoute) {
        if route != MainMenuRoute::Queue {
            self.route = route;
        }
    }

    pub fn go_home(&mut self) {
        self.route = MainMenuRoute::Home;
    }

    pub fn apply_map_catalog_custom(&mut self, catalog: &[sow_core::maps::MapCatalogEntry]) {
        let cfg = &mut self.custom_game_config;
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
    let narrow = ui.available_width() < 768.0;

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
        ui.hyperlink_to(text, "https://discord.gg/d6ZDeChSE");
    };

    let draw_github_link = |ui: &mut egui::Ui| {
        let text = egui::RichText::new("GitHub")
            .font(sow_ui_kit::theme::font_regular(size))
            .color(link_color);
        ui.hyperlink_to(text, "https://github.com/worldofunreal/shadows-of-war");
    };

    if narrow {
        ui.vertical_centered(|ui| {
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
            ui.horizontal_wrapped(|ui| {
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
        });
        return;
    }

    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 15.0),
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

/// Home shell. The identity bar stays outside the single body scroll region;
/// every control below is width-bounded by the current viewport metrics.
fn draw_home(
    root_ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    action: &mut Option<UiAction>,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    let metrics = layout::main_menu_metrics(root_ui.ctx());
    let muted = sow_ui_kit::theme::palette::text_muted();
    let header_frame = egui::Frame::NONE
        .fill(sow_ui_kit::theme::palette::surface_transparent())
        .inner_margin(egui::Margin::symmetric(metrics.outer_pad as i8, 8));
    header_frame.show(root_ui, |ui| {
        if metrics.class == layout::ViewportClass::Wide {
            ui.horizontal(|ui| {
                let actions_w = 82.0 + 92.0 + metrics.touch_min + metrics.gap * 2.0;
                let profile_w = (ui.available_width() - actions_w).max(260.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(profile_w, 56.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| profile::draw_user_profile_header(ui, state, 56.0, asset_loader, lang, action),
                );
                ui.add_space(metrics.gap);
                draw_menu_actions(ui, metrics, action);
            });
        } else {
            profile::draw_user_profile_header(
                ui,
                state,
                if metrics.is_phone() { 52.0 } else { 56.0 },
                asset_loader,
                lang,
                action,
            );
            ui.add_space(metrics.gap * 0.5);
            draw_menu_actions(ui, metrics, action);
        }
    });

    egui::ScrollArea::vertical()
        .id_salt("home_body_scroll")
        .auto_shrink([false, false])
        .show(root_ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(&strings.quick_match_label)
                    .size(12.0)
                    .strong()
                    .color(muted),
            );
            ui.add_space(4.0);

            let quick_w = if metrics.class == layout::ViewportClass::Wide {
                (ui.available_width() * 0.58).min(420.0)
            } else {
                ui.available_width()
            };
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(quick_w, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        browser::draw_left_column(
                            ui,
                            state,
                            metrics.is_compact(),
                            0.0,
                            action,
                            asset_loader,
                            lang,
                        );
                    },
                );
                if metrics.class == layout::ViewportClass::Wide {
                    ui.add_space(metrics.gap);
                    let create = crate::widgets::ThemeButton::new(&strings.create_game_btn)
                        .style(crate::widgets::ThemeButtonStyle::Secondary)
                        .min_size(egui::vec2(ui.available_width(), 52.0))
                        .text_size(18.0);
                    if ui.add(create).clicked() {
                        state.open_route(MainMenuRoute::Create);
                        state.custom_game_is_sp = false;
                    }
                }
            });

            if metrics.class != layout::ViewportClass::Wide {
                ui.add_space(metrics.gap);
                let create = crate::widgets::ThemeButton::new(&strings.create_game_btn)
                    .style(crate::widgets::ThemeButtonStyle::Secondary)
                    .min_size(egui::vec2(ui.available_width(), 52.0))
                    .text_size(18.0);
                if ui.add(create).clicked() {
                    state.open_route(MainMenuRoute::Create);
                    state.custom_game_is_sp = false;
                }
            }

            ui.add_space(metrics.gap);
            ui.label(
                egui::RichText::new(&strings.game_browser_title)
                    .size(12.0)
                    .strong()
                    .color(muted),
            );
            ui.add_space(4.0);
            join_browser::draw_filter_pills(ui, state, strings);
            ui.add_space(metrics.gap * 0.5);
            join_browser::draw_private_join_row(ui, state, strings, action);
            ui.add_space(metrics.gap);
            join_browser::draw_lobby_rows(ui, state, asset_loader, action, strings);
        });
}

fn draw_menu_actions(
    ui: &mut egui::Ui,
    metrics: layout::MainMenuMetrics,
    action: &mut Option<UiAction>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = metrics.gap;
        let store = crate::widgets::ThemeButton::new("STORE")
            .style(crate::widgets::ThemeButtonStyle::Secondary)
            .min_size(egui::vec2(82.0, metrics.touch_min))
            .text_size(12.0);
        if ui.add(store).clicked() {
            #[cfg(target_os = "ios")]
            { *action = Some(UiAction::OpenStore); }
            #[cfg(not(target_os = "ios"))]
            { *action = Some(UiAction::OpenStorePage); }
        }
        let profile = crate::widgets::ThemeButton::new("PROFILE")
            .style(crate::widgets::ThemeButtonStyle::Tertiary)
            .min_size(egui::vec2(92.0, metrics.touch_min))
            .text_size(11.0);
        if ui.add(profile).clicked() {
            *action = Some(UiAction::OpenProfilePage);
        }
        let gear = crate::widgets::ThemeButton::new("⚙")
            .style(crate::widgets::ThemeButtonStyle::Tertiary)
            .min_size(egui::vec2(metrics.touch_min, metrics.touch_min))
            .text_size(24.0);
        if ui.add(gear).clicked() {
            *action = Some(UiAction::ToggleSettings);
        }
    });
}

pub fn draw(
    root_ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    reduced_motion: bool,
) -> Option<UiAction> {
    let mut action = None;
    let metrics = layout::main_menu_metrics(root_ui.ctx());
    let compact = metrics.is_compact();
    let strings = &sow_i18n::get(lang).main_menu;

    // Draw the full-bleed backdrop first so that all panels (including the footer)
    // are drawn on top of it.
    if matches!(state.visible_route(), MainMenuRoute::Home | MainMenuRoute::Browser | MainMenuRoute::Queue)
        && !state.show_leader_picker
    {
        let backdrop_rect = root_ui.ctx().content_rect();
        let use_portrait = backdrop_rect.width() < backdrop_rect.height();
        crate::widgets::draw_leader_hero_backdrop(
            root_ui,
            &mut crate::widgets::LeaderHeroBackdropCtx {
                screen_rect: backdrop_rect,
                selected: state.selected_leader,
                mobile: use_portrait,
                asset_loader,
                transition: &mut state.leader_backdrop,
                loading_label: &strings.loading_leader_portrait,
                draw_picker_gradient: false,
            },
        );
    }

    egui::Panel::bottom("main_menu_footer_panel")
        .frame(
            egui::Frame::NONE
                .fill(sow_ui_kit::theme::palette::surface())
                .inner_margin(egui::Margin::symmetric(16, 4)),
        )
        .show_inside(root_ui, |ui| {
            draw_terms_privacy_footer(ui, lang, &mut action);
        });

    match state.visible_route() {
        MainMenuRoute::Store => store::draw(root_ui, state, asset_loader, &mut action),
        MainMenuRoute::Profile => profile::draw_native(root_ui, state, &mut action),
        MainMenuRoute::Create => custom_game::draw(
            root_ui,
            state,
            asset_loader,
            &mut action,
            lang,
            reduced_motion,
        ),
        MainMenuRoute::Queue => {
            let (_, action_min_h, _, _) = layout::menu_layout_chrome(
                root_ui.ctx(),
                root_ui.available_height(),
                root_ui.available_width(),
                compact,
            );
            queue_overlay::draw_queue_overlay(
                root_ui,
                state,
                action_min_h,
                &mut action,
                asset_loader,
                lang,
            );
        }
        MainMenuRoute::Browser => draw_browser(root_ui, state, asset_loader, lang, &mut action),
        MainMenuRoute::Home => draw_home(root_ui, state, asset_loader, lang, &mut action),
    }

    modals::draw_connecting_indicator(root_ui.ctx(), state, lang, compact);
    modals::draw_map_download_indicator(root_ui.ctx(), state, lang, compact);

    // Password prompt for joining a protected Public Games lobby (inline browser).
    if let Some(target_id) = state.join_password_for_lobby {
        join_browser::draw_password_modal(root_ui, state, target_id, &mut action, strings, compact);
    }

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

fn draw_browser(
    root_ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    action: &mut Option<UiAction>,
) {
    let metrics = layout::main_menu_metrics(root_ui.ctx());
    let strings = &sow_i18n::get(lang).main_menu;
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(metrics.outer_pad as i8, 10))
        .show(root_ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(crate::widgets::ThemeButton::new("← BACK")
                    .style(crate::widgets::ThemeButtonStyle::Tertiary)
                    .min_size(egui::vec2(92.0, metrics.touch_min))
                    .text_size(13.0))
                    .clicked()
                {
                    state.go_home();
                }
                ui.label(egui::RichText::new("PUBLIC GAMES").strong().size(22.0));
            });
            ui.add_space(metrics.gap);
            egui::ScrollArea::vertical()
                .id_salt("browser_body_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    join_browser::draw_filter_pills(ui, state, strings);
                    ui.add_space(metrics.gap * 0.5);
                    join_browser::draw_private_join_row(ui, state, strings, action);
                    ui.add_space(metrics.gap);
                    join_browser::draw_lobby_rows(ui, state, asset_loader, action, strings);
                });
        });
}
