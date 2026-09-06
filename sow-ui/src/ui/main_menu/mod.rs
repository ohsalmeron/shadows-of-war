pub mod browser;
pub mod custom_game;
pub mod join_browser;
mod layout;
#[cfg(test)]
mod layout_tests;
mod modals;
pub mod profile;
pub mod queue_overlay;
pub(crate) mod shell;
pub mod store;
mod topbar;

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
    Heroes,
    Store,
    Profile,
}

/// Destinations exposed by the persistent native menu navigation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainMenuSection {
    Battle,
    Heroes,
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
    /// Last valid matchmaking lobby shown on the home screen. A connected
    /// client may receive an empty snapshot while the server rotates lobbies;
    /// keep that transition invisible until the replacement arrives.
    pub last_matchmaking_lobby: Option<LobbyInfo>,
    /// Local countdown anchor for the home card: (lobby id, server seconds,
    /// local time at which that server value was received/observed).
    pub matchmaking_countdown_anchor: Option<(u64, f32, f64)>,
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
            last_matchmaking_lobby: None,
            matchmaking_countdown_anchor: None,
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
        if self.is_waiting
            && matches!(
                self.route,
                MainMenuRoute::Home
                    | MainMenuRoute::Browser
                    | MainMenuRoute::Create
                    | MainMenuRoute::Queue
            )
        {
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

    pub fn active_section(&self) -> MainMenuSection {
        match self.visible_route() {
            MainMenuRoute::Heroes => MainMenuSection::Heroes,
            MainMenuRoute::Store => MainMenuSection::Store,
            MainMenuRoute::Profile => MainMenuSection::Profile,
            MainMenuRoute::Home
            | MainMenuRoute::Browser
            | MainMenuRoute::Create
            | MainMenuRoute::Queue => MainMenuSection::Battle,
        }
    }

    pub fn open_section(&mut self, section: MainMenuSection) {
        self.route = match section {
            MainMenuSection::Battle => MainMenuRoute::Home,
            MainMenuSection::Heroes => MainMenuRoute::Heroes,
            MainMenuSection::Store => MainMenuRoute::Store,
            MainMenuSection::Profile => MainMenuRoute::Profile,
        };
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
        // Mobile has only one footer row. Wrapping here increases the reserved
        // bottom-panel height and steals the space needed by the fixed home UI.
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            draw_terms_link(ui, action);
            ui.label(
                egui::RichText::new("·")
                    .font(sow_ui_kit::theme::font_regular(9.0))
                    .color(text_color),
            );
            draw_privacy_link(ui, action);
            ui.label(
                egui::RichText::new("·")
                    .font(sow_ui_kit::theme::font_regular(9.0))
                    .color(text_color),
            );
            ui.label(
                egui::RichText::new(&version)
                    .font(sow_ui_kit::theme::font_regular(9.0))
                    .color(text_color),
            );
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

fn draw_home_content(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    strings: &sow_i18n::MainMenuStrings,
    metrics: layout::MainMenuMetrics,
    action: &mut Option<UiAction>,
) {
    // Warm every eligible rotation thumbnail while the current lobby is still
    // on screen, so a new lobby never appears before its preview is decoded.
    asset_loader.prefetch_matchmaking_thumbnails();

    let portrait = metrics.is_phone();
    let mut body = ui.available_rect_before_wrap();
    body.min.x += metrics.outer_pad;
    body.max.x -= metrics.outer_pad;
    body.min.y += 8.0;
    body.max.y -= 12.0;
    let body_height = body.height().max(0.0);

    // Choose the map size once; the frame and every action use that same
    // content width. Frame padding (16) and stroke (1) add 17 per side.
    let reserved = if body_height < 760.0 { 270.0 } else { 330.0 };
    let map_height_cap = if portrait { 190.0 } else { 560.0 * 9.0 / 16.0 };
    let map_width = (body.width() - 34.0)
        .max(0.0)
        .min((body_height - reserved).max(0.0).min(map_height_cap) * (16.0 / 9.0));
    let panel_width = map_width + 34.0;
    let panel_x = if portrait {
        body.center().x - panel_width * 0.5
    } else {
        body.left()
    };
    let panel_bounds = egui::Rect::from_min_size(
        egui::pos2(panel_x, body.top()),
        egui::vec2(panel_width, body_height),
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(panel_bounds)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(panel_bounds));
            draw_command_panel(ui, state, asset_loader, lang, strings, body_height, action);
        },
    );
}

fn draw_command_panel(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    strings: &sow_i18n::MainMenuStrings,
    body_height: f32,
    action: &mut Option<UiAction>,
) {
    let dense = body_height < 760.0;
    let control_h = if dense { 40.0 } else { 44.0 };
    let vertical_gap = 0.0;
    egui::Frame::NONE
        .fill(egui::Color32::from_rgba_unmultiplied(9, 11, 15, 190))
        .stroke(egui::Stroke::new(
            1.0_f32,
            sow_ui_kit::theme::palette::field_border(),
        ))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(16, if dense { 10 } else { 14 }))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = vertical_gap;
            browser::draw_left_column(ui, state, true, 0.0, action, asset_loader, lang);
            ui.add_space(vertical_gap);

            let browser = crate::widgets::ThemeButton::new("LOBBY BROWSER  →")
                .style(crate::widgets::ThemeButtonStyle::Tertiary)
                .min_size(egui::vec2(ui.available_width(), control_h))
                .text_size(12.0);
            if ui.add(browser).clicked() {
                *action = Some(UiAction::OpenJoinBrowser);
            }
            ui.add_space(vertical_gap);
            join_browser::draw_private_join_row(ui, state, strings, action);
            ui.add_space(vertical_gap);

            let create = crate::widgets::ThemeButton::new("CREATE CUSTOM GAME  +")
                .style(crate::widgets::ThemeButtonStyle::Tertiary)
                .min_size(egui::vec2(ui.available_width(), control_h))
                .text_size(12.0);
            if ui.add(create).clicked() {
                state.open_route(MainMenuRoute::Create);
                state.custom_game_is_sp = false;
            }
            ui.add_space(vertical_gap);
            draw_store_home_button(ui, control_h, action);
        });
}

fn draw_store_home_button(ui: &mut egui::Ui, height: f32, action: &mut Option<UiAction>) {
    let store = crate::widgets::ThemeButton::new("STORE  ↗")
        .style(crate::widgets::ThemeButtonStyle::Tertiary)
        .min_size(egui::vec2(ui.available_width(), height))
        .text_size(15.0);
    if ui.add(store).clicked() {
        #[cfg(target_os = "ios")]
        {
            *action = Some(UiAction::OpenStore);
        }
        #[cfg(not(target_os = "ios"))]
        {
            *action = Some(UiAction::OpenStorePage);
        }
    }
}

pub fn draw(
    root_ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    reduced_motion: bool,
) -> Option<UiAction> {
    shell::draw(root_ui, state, asset_loader, lang, reduced_motion)
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
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("BATTLE / LOBBY BROWSER")
                        .strong()
                        .size(if metrics.is_phone() { 18.0 } else { 24.0 })
                        .color(sow_ui_kit::theme::palette::neon_cyan()),
                );
            });
            ui.add_space(metrics.gap);
            egui::ScrollArea::vertical()
                .id_salt("browser_body_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    join_browser::draw_private_join_row(ui, state, strings, action);
                    ui.add_space(metrics.gap);
                    join_browser::draw_lobby_rows(ui, state, asset_loader, action, strings);
                });
        });
}
