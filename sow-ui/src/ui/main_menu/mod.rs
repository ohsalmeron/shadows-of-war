pub mod browser;
pub mod custom_game;
pub mod join_browser;
mod layout;
mod modals;
pub mod profile;
pub mod queue_overlay;

use crate::UiAction;
use egui::{CentralPanel, Color32, Frame};
use sow_core::protocol::LobbyInfo;

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
    pub show_custom_game: bool,
    pub custom_game_is_sp: bool,
    pub custom_game_config: Box<sow_core::game_config::GameConfig>,
    pub custom_game_password: String,
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
    pub error_message: Option<String>,
    pub leader_backdrop: crate::widgets::LeaderBackdropTransition,
    /// Local player's id in the joined lobby — lets the host roster skip its own entry.
    pub my_player_id: Option<u16>,
    /// Active lobby notice (host left / kicked / banned) and the frame time it appeared.
    pub notice: Option<LobbyNotice>,
    pub notice_at: Option<f64>,
    pub safe_area_bottom: f32,
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
            show_custom_game: false,
            custom_game_is_sp: true,
            custom_game_config: Box::new(sow_core::game_config::GameConfig {
                seed: ms as u64,
                ..Default::default()
            }),
            custom_game_password: String::new(),
            custom_game_is_private: false,
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
            leader_backdrop: crate::widgets::LeaderBackdropTransition::new(leader),
            error_message: None,
            my_player_id: None,
            notice: None,
            notice_at: None,
            safe_area_bottom: 0.0,
        }
    }
}

impl MainMenuState {
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
        ui.hyperlink_to(text, "https://discord.gg/eauHRf7zP");
    };

    let draw_github_link = |ui: &mut egui::Ui| {
        let text = egui::RichText::new("GitHub")
            .font(sow_ui_kit::theme::font_regular(size))
            .color(link_color);
        ui.hyperlink_to(text, "https://github.com/ohsalmeron/shadows-of-war");
    };

    if narrow {
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 15.0),
                egui::Layout::left_to_right(egui::Align::Center)
                    .with_main_align(egui::Align::Center)
                    .with_main_wrap(true),
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
                },
            );
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 15.0),
                egui::Layout::left_to_right(egui::Align::Center)
                    .with_main_align(egui::Align::Center)
                    .with_main_wrap(true),
                |ui| {
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
                },
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

/// Home screen, built from fixed panels so the chrome never scrolls — the only scroll
/// region is the Public Games list in the middle of the central panel. The selected
/// leader is already conveyed by the full-bleed backdrop and the identity avatar, so
/// there is no separate leader card competing for space.
///
/// Layout (the footer bottom panel is drawn by the caller before this):
/// ```text
///   ┌ identity (Panel::top, fixed) ───────────────────────┐
///   │ [avatar] name / sign-in                      [gear] │
///   ├─────────────────────────────────────────────────────┤
///   │ CENTRAL (dark)                                       │
///   │  Panel::top → [ Quick Match card ][  CREATE  ]       │  (desktop row)
///   │             → Public Games  [All][FFA][Teams]        │
///   │  ScrollArea → lobby list ↕                           │
///   │  Panel::bot → room code                              │
///   └─────────────────────────────────────────────────────┘
/// ```
/// On mobile the top row stacks: Quick Match card, then a full-width CREATE.
fn draw_home(
    root_ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    action: &mut Option<UiAction>,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    let compact = sow_ui_kit::theme::compact_viewport(root_ui.ctx());
    let scale = sow_ui_kit::theme::viewport_scale(root_ui.ctx());
    let section_gap = 12.0 * scale;
    let muted = sow_ui_kit::theme::palette::text_muted();
    // Wide enough to place the CREATE button beside the Quick Match card.
    let wide = root_ui.available_width() > 760.0;

    // ── Identity strip (fixed, full width): avatar + name/sign-in + settings gear ──
    egui::Panel::top("home_identity")
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.inner_margin(egui::Margin {
            left: 16,
            right: 16,
            top: 16,
            bottom: 8,
        }))
        .show_inside(root_ui, |ui| {
            ui.horizontal(|ui| {
                let gear_w = 44.0;
                let header_w = (ui.available_width() - gear_w - 8.0).max(80.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(header_w, 48.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        profile::draw_user_profile_header(
                            ui,
                            state,
                            compact,
                            56.0,
                            asset_loader,
                            lang,
                            action,
                        );
                    },
                );
                let gear = crate::widgets::ThemeButton::new("⚙")
                    .style(crate::widgets::ThemeButtonStyle::Tertiary)
                    .min_size(egui::vec2(gear_w, 44.0))
                    .text_size(20.0);
                if ui.add(gear).clicked() {
                    *action = Some(UiAction::ToggleSettings);
                }
            });
        });

    // ── Central: Quick Match + Public Games (only the lobby list scrolls) ──
    // No panel fill — the leader backdrop shows through; individual cards/rows carry
    // their own surfaces.
    CentralPanel::default()
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(16, 12)))
        .show_inside(root_ui, |ui| {
            // Fixed top region: Quick Match card, then the Public Games header + pills.
            egui::Panel::top("home_pg_top")
                .resizable(false)
                .show_separator_line(false)
                .frame(egui::Frame::NONE)
                .show_inside(ui, |ui| {
                    ui.label(
                        egui::RichText::new(&strings.quick_match_label)
                            .size(12.0)
                            .strong()
                            .color(muted),
                    );
                    ui.add_space(4.0);

                    // Quick Match card, left-aligned at a compact hero size.
                    let card_w = if wide { 320.0 } else { ui.available_width() };
                    ui.allocate_ui_with_layout(
                        egui::vec2(card_w, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            browser::draw_left_column(
                                ui,
                                state,
                                compact,
                                0.0,
                                action,
                                asset_loader,
                                lang,
                            );
                        },
                    );

                    ui.add_space(section_gap);
                    ui.label(
                        egui::RichText::new(&strings.game_browser_title)
                            .size(12.0)
                            .strong()
                            .color(muted),
                    );
                    ui.add_space(4.0);
                    join_browser::draw_filter_pills(ui, state, strings);
                    ui.add_space(6.0);
                });

            // Fixed bottom action bar: [ lobby-code component ] [ CREATE ] — mirrors the
            // identity bar's [ nickname box ] [ gear ] rhythm. Stacks on narrow screens.
            egui::Panel::bottom("home_pg_bottom")
                .resizable(false)
                .show_separator_line(false)
                .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                    left: 0,
                    right: 0,
                    top: 10,
                    bottom: 0,
                }))
                .show_inside(ui, |ui| {
                    let bar_h = 46.0;
                    let mut open_create = false;
                    if wide {
                        ui.horizontal(|ui| {
                            let create_w = 200.0;
                            let comp_w = (ui.available_width() - create_w - section_gap).max(160.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(comp_w, bar_h),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    join_browser::draw_private_join_row(ui, state, strings, action);
                                },
                            );
                            ui.add_space(section_gap);
                            let create_btn =
                                crate::widgets::ThemeButton::new(&strings.create_game_btn)
                                    .style(crate::widgets::ThemeButtonStyle::Secondary)
                                    .min_size(egui::vec2(ui.available_width(), bar_h))
                                    .text_size(18.0);
                            open_create = ui.add(create_btn).clicked();
                        });
                    } else {
                        join_browser::draw_private_join_row(ui, state, strings, action);
                        ui.add_space(section_gap * 0.5);
                        let create_btn = crate::widgets::ThemeButton::new(&strings.create_game_btn)
                            .style(crate::widgets::ThemeButtonStyle::Secondary)
                            .min_size(egui::vec2(ui.available_width(), bar_h))
                            .text_size(18.0);
                        open_create = ui.add(create_btn).clicked();
                    }
                    if open_create {
                        state.show_custom_game = true;
                        state.custom_game_is_sp = false;
                    }
                });

            // Remaining middle: the scrolling lobby list — the only scroll on the menu.
            egui::ScrollArea::vertical()
                .id_salt("home_public_games_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    join_browser::draw_lobby_rows(ui, state, asset_loader, action, strings);
                });
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
    let compact = sow_ui_kit::theme::compact_viewport(root_ui.ctx());
    let strings = &sow_i18n::get(lang).main_menu;

    // Draw the full-bleed backdrop first so that all panels (including the footer)
    // are drawn on top of it.
    if (state.is_waiting || !state.show_custom_game) && !state.show_leader_picker {
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

    if state.show_custom_game && !state.is_waiting {
        custom_game::draw(
            root_ui,
            state,
            asset_loader,
            &mut action,
            lang,
            reduced_motion,
        );
    } else if state.is_waiting {
        // Lobby waiting room fills the space between the top and the (already-drawn) footer.
        CentralPanel::default()
            .frame(Frame::new().fill(Color32::TRANSPARENT))
            .show_inside(root_ui, |ui| {
                let (_, action_min_h, _, _) = layout::menu_layout_chrome(
                    ui.ctx(),
                    ui.available_height(),
                    ui.available_width(),
                    compact,
                );
                queue_overlay::draw_queue_overlay(
                    ui,
                    state,
                    action_min_h,
                    &mut action,
                    asset_loader,
                    lang,
                );
            });
    } else {
        draw_home(root_ui, state, asset_loader, lang, &mut action);
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
