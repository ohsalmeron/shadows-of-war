pub mod actions;
pub mod browser;
pub mod create_game;
pub mod join_browser;
pub mod profile;
pub mod queue_overlay;
pub mod single_player_setup;

use crate::UiAction;
use egui::{Align, CentralPanel, Color32, Frame, Layout, RichText, Stroke};
use sow_core::protocol::LobbyInfo;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GameModeFilter {
    #[default]
    All,
    Ffa,
    Teams,
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

fn menu_footer_height(section_gap: f32, action_min_h: f32, scale: f32) -> f32 {
    let settings_h = action_min_h * 0.75;
    action_min_h // Solo button
        + section_gap
        + action_min_h // Create button
        + section_gap
        + action_min_h // Join button
        + section_gap
        + settings_h // Settings button
        + 6.0 * scale
}

fn menu_layout_chrome(
    ctx: &egui::Context,
    panel_h: f32,
    available_w: f32,
    compact: bool,
) -> (f32, f32, f32, f32) {
    let scale = crate::ui::theme::viewport_scale(ctx);
    let portrait = crate::ui::theme::portrait_layout(ctx);
    let mut section_gap = (if portrait {
        8.0
    } else if compact {
        12.0
    } else {
        16.0
    }) * scale;
    let mut action_min_h = (if portrait {
        54.0
    } else if compact {
        64.0
    } else {
        72.0
    }) * scale;
    let mut profile_height = 56.0 * scale;
    if portrait {
        profile_height *= 0.85;
    }

    let mut lobby_h = crate::ui::map_texture::thumbnail_square_side(available_w, compact);
    if portrait {
        lobby_h = (lobby_h * 0.55).clamp(110.0, 160.0);
    }

    let footer_h = menu_footer_height(section_gap, action_min_h, scale);
    let header_h = profile_height + section_gap;
    let needed = header_h + section_gap + footer_h + section_gap + lobby_h;
    let shrink = crate::ui::theme::fit_scale(needed, panel_h);
    if shrink < 1.0 {
        section_gap *= shrink;
        action_min_h *= shrink;
        profile_height *= shrink;
        lobby_h *= shrink;
    }
    (section_gap, action_min_h, profile_height, lobby_h)
}

#[allow(clippy::too_many_arguments)]
fn draw_menu_footer(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    section_gap: f32,
    action_min_h: f32,
    compact: bool,
    action: &mut Option<UiAction>,
    lang: sow_i18n::Language,
) {
    actions::draw_right_column(ui, state, section_gap, action_min_h, compact, action, lang);
}

#[allow(clippy::too_many_arguments)]
fn draw_menu_right_panel_contents(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    section_gap: f32,
    action_min_h: f32,
    compact: bool,
    profile_height: f32,
    lobby_height: f32,
    panel_inner_h: f32,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
) {
    let portrait = !compact || crate::ui::theme::portrait_layout(ui.ctx());
    let landscape_compact = compact && !portrait;
    let panel_w = ui.available_width();
    let header_h = profile_height + section_gap;

    let panel_rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(panel_w, panel_inner_h));

    ui.scope_builder(egui::UiBuilder::new().max_rect(panel_rect), |ui| {
        profile::draw_user_profile_header(
            ui,
            state,
            compact,
            profile_height,
            asset_loader,
            lang,
            action,
        );
        ui.add_space(section_gap);

        if landscape_compact {
            let row_h = (panel_inner_h - header_h).max(0.0);
            let action_col_w = (panel_w * 0.38).clamp(120.0, 160.0);
            let lobby_w = (panel_w - action_col_w - section_gap).max(80.0);

            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(lobby_w, row_h),
                    Layout::top_down(Align::Min),
                    |ui| {
                        browser::draw_left_column(
                            ui,
                            state,
                            section_gap,
                            action_min_h,
                            compact,
                            row_h,
                            action,
                            asset_loader,
                            lang,
                        );
                    },
                );
                ui.add_space(section_gap);
                ui.allocate_ui_with_layout(
                    egui::vec2(action_col_w, row_h),
                    Layout::top_down(Align::Min),
                    |ui| {
                        draw_menu_footer(
                            ui,
                            state,
                            section_gap,
                            action_min_h,
                            compact,
                            action,
                            lang,
                        );
                    },
                );
            });
        } else {
            browser::draw_left_column(
                ui,
                state,
                section_gap,
                action_min_h,
                compact,
                lobby_height,
                action,
                asset_loader,
                lang,
            );

            ui.add_space(section_gap);

            draw_menu_footer(ui, state, section_gap, action_min_h, compact, action, lang);
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn draw_menu_right_panel(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    section_gap: f32,
    action_min_h: f32,
    compact: bool,
    profile_height: f32,
    lobby_height: f32,
    panel_outer_h: f32,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
) {
    let panel_w = ui.available_width();

    ui.allocate_ui_with_layout(
        egui::vec2(panel_w, panel_outer_h),
        Layout::top_down(Align::Min),
        |ui| {
            if compact {
                draw_menu_right_panel_contents(
                    ui,
                    state,
                    section_gap,
                    action_min_h,
                    compact,
                    profile_height,
                    lobby_height,
                    panel_outer_h,
                    action,
                    asset_loader,
                    lang,
                );
            } else {
                crate::ui::theme::menu_right_panel_frame(false).show(ui, |ui| {
                    let h = ui.available_height();
                    ui.set_min_height(h);
                    draw_menu_right_panel_contents(
                        ui,
                        state,
                        section_gap,
                        action_min_h,
                        compact,
                        profile_height,
                        lobby_height,
                        h,
                        action,
                        asset_loader,
                        lang,
                    );
                });
            }
        },
    );
}

fn draw_indicator_toast(
    ctx: &egui::Context,
    id: &str,
    pad_y: f32,
    label: &str,
    color: Color32,
    compact: bool,
) {
    let pad_x = if compact { 24.0 } else { 20.0 };
    egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-pad_x, pad_y))
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(Color32::from_black_alpha(140))
                .corner_radius(8)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(egui::RichText::new(label).color(color).size(if compact {
                            13.0
                        } else {
                            14.0
                        }));
                    });
                });
        });
}

fn draw_map_download_indicator(
    ctx: &egui::Context,
    state: &MainMenuState,
    lang: sow_i18n::Language,
    compact: bool,
) {
    if !state.is_downloading_map {
        return;
    }
    let map_name = state.downloading_map_name.as_deref().unwrap_or("map");
    let strings = &sow_i18n::get(lang).main_menu;
    let label = strings
        .downloading_map
        .replacen("{}", map_name, 1)
        .replacen("{}", &state.map_download_progress.to_string(), 1);
    let pad_y = if compact { 96.0 } else { 56.0 };
    draw_indicator_toast(
        ctx,
        "main_menu_map_download",
        pad_y,
        &label,
        crate::ui::theme::palette::neon_cyan(),
        compact,
    );
}

fn draw_connecting_indicator(
    ctx: &egui::Context,
    state: &MainMenuState,
    lang: sow_i18n::Language,
    compact: bool,
) {
    if state.is_connected {
        return;
    }
    let strings = &sow_i18n::get(lang).main_menu;
    let pad_y = if compact { 56.0 } else { 20.0 };
    draw_indicator_toast(
        ctx,
        "main_menu_connecting",
        pad_y,
        &strings.connecting,
        crate::ui::theme::palette::text_muted(),
        compact,
    );
}

fn draw_link_conflict_modal(
    root_ui: &mut egui::Ui,
    action: &mut Option<UiAction>,
    conflict: &UiLinkConflictInfo,
    lang: sow_i18n::Language,
    compact: bool,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    let screen_rect = root_ui.ctx().content_rect();
    let modal_w = if compact {
        screen_rect.width() - 32.0
    } else {
        480.0
    };

    egui::Area::new(egui::Id::new("link_conflict_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(root_ui.ctx(), |ui| {
            ui.painter()
                .rect_filled(screen_rect, 0.0, crate::ui::theme::palette::backdrop());
        });

    egui::Window::new(&strings.link_conflict_title)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .fixed_size(egui::vec2(modal_w, if compact { 360.0 } else { 320.0 }))
        .frame(
            Frame::new()
                .fill(crate::ui::theme::palette::surface())
                .stroke(Stroke::new(
                    1.5_f32,
                    crate::ui::theme::palette::neon_cyan_hover(),
                ))
                .corner_radius(egui::CornerRadius::same(16))
                .inner_margin(24.0)
                .shadow(egui::Shadow {
                    blur: 32,
                    spread: 0,
                    color: crate::ui::theme::palette::neon_cyan().linear_multiply(0.2),
                    offset: [0, 8],
                }),
        )
        .show(root_ui.ctx(), |ui| {
            ui.set_width(modal_w - 48.0);
            ui.vertical_centered(|ui| {
                crate::ui::theme::outlined_label(
                    ui,
                    &strings.link_conflict_title,
                    egui::FontId::proportional(20.0),
                    crate::ui::theme::palette::neon_cyan(),
                );
                ui.add_space(12.0);
                ui.label(
                    RichText::new(&strings.link_conflict_body)
                        .size(13.0)
                        .color(crate::ui::theme::palette::text_muted()),
                );
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    let col_w = (ui.available_width() - 12.0) / 2.0;
                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        ui.label(
                            RichText::new(&strings.link_conflict_guest_label)
                                .strong()
                                .color(crate::ui::theme::palette::neon_cyan()),
                        );
                        ui.label(
                            RichText::new(
                                strings
                                    .link_conflict_level
                                    .replace("{}", &conflict.current_level.to_string()),
                            )
                            .size(16.0),
                        );
                    });
                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        ui.label(
                            RichText::new(&strings.link_conflict_platform_label)
                                .strong()
                                .color(crate::ui::theme::palette::neon_cyan()),
                        );
                        ui.label(
                            RichText::new(
                                strings
                                    .link_conflict_level
                                    .replace("{}", &conflict.existing_level.to_string()),
                            )
                            .size(16.0),
                        );
                    });
                });
                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    let btn_w = if compact {
                        ui.available_width()
                    } else {
                        (ui.available_width() - 12.0) / 2.0
                    };
                    let guest_btn =
                        crate::widgets::ThemeButton::new(&strings.link_conflict_keep_guest)
                            .style(crate::widgets::ThemeButtonStyle::Secondary)
                            .min_size(egui::vec2(btn_w, 40.0));
                    if ui.add(guest_btn).clicked() {
                        *action = Some(UiAction::ResolveLinkConflict {
                            keep_account_id: conflict.current_account_id.clone(),
                        });
                    }
                    if !compact {
                        ui.add_space(12.0);
                    } else {
                        ui.add_space(8.0);
                    }
                    let platform_btn =
                        crate::widgets::ThemeButton::new(&strings.link_conflict_keep_platform)
                            .style(crate::widgets::ThemeButtonStyle::Primary)
                            .min_size(egui::vec2(btn_w, 40.0));
                    if ui.add(platform_btn).clicked() {
                        *action = Some(UiAction::ResolveLinkConflict {
                            keep_account_id: conflict.existing_account_id.clone(),
                        });
                    }
                });
            });
        });
}

pub fn draw_terms_privacy_footer(
    ui: &mut egui::Ui,
    lang: sow_i18n::Language,
    action: &mut Option<UiAction>,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    let version = format!("v{}", include_str!("../../../../.version").trim());
    let credits = &sow_i18n::get(lang).credits;
    let text_color = crate::ui::theme::palette::text_muted();
    let link_color = crate::ui::theme::palette::neon_cyan();
    let size = 11.0;
    let narrow = ui.available_width() < 480.0;

    let draw_terms_link = |ui: &mut egui::Ui, action: &mut Option<UiAction>| {
        let terms_id = ui.make_persistent_id("terms_of_service_link");
        let terms_hover_t = ui.ctx().animate_bool(terms_id, false);
        let mut terms_text = egui::RichText::new(&sow_i18n::get(lang).settings.terms_of_service)
            .font(crate::ui::theme::font_regular(size))
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
            .font(crate::ui::theme::font_regular(size))
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

    if narrow {
        ui.with_layout(
            egui::Layout::top_down(egui::Align::Center).with_cross_align(egui::Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(
                        egui::RichText::new(strings.by_playing_you_agree.trim_end())
                            .font(crate::ui::theme::font_regular(size))
                            .color(text_color),
                    );
                    draw_terms_link(ui, action);
                    ui.label(
                        egui::RichText::new(strings.and_the.trim())
                            .font(crate::ui::theme::font_regular(size))
                            .color(text_color),
                    );
                    draw_privacy_link(ui, action);
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(
                        egui::RichText::new(&version)
                            .font(crate::ui::theme::font_regular(size))
                            .color(text_color),
                    );
                    ui.label(
                        egui::RichText::new("·")
                            .font(crate::ui::theme::font_regular(size))
                            .color(text_color),
                    );
                    ui.label(
                        egui::RichText::new(&credits.based_on_short)
                            .font(crate::ui::theme::font_regular(size))
                            .color(text_color),
                    );
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
                    .font(crate::ui::theme::font_regular(size))
                    .color(text_color),
            );

            draw_terms_link(ui, action);

            ui.label(
                egui::RichText::new(strings.and_the.trim())
                    .font(crate::ui::theme::font_regular(size))
                    .color(text_color),
            );

            draw_privacy_link(ui, action);

            ui.label(
                egui::RichText::new("·")
                    .font(crate::ui::theme::font_regular(size))
                    .color(text_color),
            );

            ui.label(
                egui::RichText::new(&version)
                    .font(crate::ui::theme::font_regular(size))
                    .color(text_color),
            );

            ui.label(
                egui::RichText::new("·")
                    .font(crate::ui::theme::font_regular(size))
                    .color(text_color),
            );

            ui.label(
                egui::RichText::new(&credits.based_on_short)
                    .font(crate::ui::theme::font_regular(size))
                    .color(text_color),
            );
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
    let compact = crate::ui::theme::compact_viewport(root_ui.ctx());
    let outer_pad = 16.0;
    let strings = &sow_i18n::get(lang).main_menu;

    // Draw the terms/privacy footer always at the bottom of the screen
    egui::Panel::bottom("main_menu_footer_panel")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(16, 8)))
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
                let screen_rect = ui.ctx().content_rect();
                if !state.show_leader_picker {
                    let use_portrait = screen_rect.width() < screen_rect.height();
                    crate::widgets::draw_leader_hero_backdrop(
                        ui,
                        screen_rect,
                        state.selected_leader,
                        use_portrait,
                        asset_loader,
                        &mut state.leader_backdrop,
                        &strings.loading_leader_portrait,
                        false,
                    );
                }

                if state.is_waiting {
                    if crate::ui::theme::lobby_modal_embed(ui.ctx()) {
                        // Hero backdrop only; lobby content in root-level modal.
                    } else {
                        let (section_gap, action_min_h, _, _) = menu_layout_chrome(
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
                    }
                } else {
                    let panel_w = crate::ui::theme::menu_rail_panel_width(
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
                        menu_layout_chrome(ui.ctx(), effective_h, inner_w, compact);

                    ui.allocate_ui_with_layout(
                        egui::vec2(panel_w, content_h),
                        Layout::top_down(Align::Min),
                        |ui| {
                            draw_menu_right_panel(
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

    if state.is_waiting && crate::ui::theme::lobby_modal_embed(root_ui.ctx()) {
        queue_overlay::draw_lobby_embed_modal(
            root_ui.ctx(),
            state,
            &mut action,
            asset_loader,
            lang,
            reduced_motion,
        );
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
        draw_link_conflict_modal(root_ui, &mut action, &conflict, lang, compact);
    }

    draw_connecting_indicator(root_ui.ctx(), state, lang, compact);
    draw_map_download_indicator(root_ui.ctx(), state, lang, compact);

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

    if let Some(err_msg) = &state.error_message {
        let mut clear_error = false;
        let mut retry = false;

        // 1. Draw a full-screen backdrop to dim the background and intercept clicks
        egui::Area::new(egui::Id::new("error_modal_backdrop"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(root_ui.ctx(), |ui| {
                let screen_rect = ui.ctx().content_rect();
                ui.painter()
                    .rect_filled(screen_rect, 0.0, crate::ui::theme::palette::backdrop());
            });

        // 2. Draw responsive centered window on top
        let screen_rect = root_ui.ctx().content_rect();
        let is_mobile = compact;
        let modal_w = if is_mobile {
            screen_rect.width() - 32.0
        } else {
            420.0
        };

        egui::Window::new(&strings.connection_error_title)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .fixed_size(egui::vec2(modal_w, 280.0))
            .frame(
                egui::Frame::new()
                    .fill(crate::ui::theme::palette::surface())
                    .stroke(egui::Stroke::new(
                        1.5_f32,
                        crate::ui::theme::palette::danger_border(),
                    ))
                    .corner_radius(egui::CornerRadius::same(16))
                    .inner_margin(24.0)
                    .shadow(egui::Shadow {
                        blur: 32,
                        spread: 0,
                        color: crate::ui::theme::palette::danger().linear_multiply(0.2),
                        offset: [0, 8],
                    }),
            )
            .show(root_ui.ctx(), |ui| {
                ui.set_width(modal_w - 48.0); // account for inner margin
                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);

                    // Outlined Warning Icon
                    crate::ui::theme::outlined_label(
                        ui,
                        "⚠️",
                        egui::FontId::proportional(36.0),
                        crate::ui::theme::palette::danger(),
                    );

                    ui.add_space(12.0);

                    // Outlined Game-Themed Title
                    crate::ui::theme::outlined_label(
                        ui,
                        &strings.connection_error_header,
                        egui::FontId::proportional(22.0),
                        crate::ui::theme::palette::danger(),
                    );

                    ui.add_space(16.0);

                    ui.label(
                        egui::RichText::new(err_msg.as_str())
                            .size(14.0)
                            .color(crate::ui::theme::palette::text_muted()),
                    );

                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(&strings.connection_error_hint)
                            .size(12.0)
                            .color(crate::ui::theme::palette::text_muted()),
                    );

                    ui.add_space(24.0);

                    ui.horizontal(|ui| {
                        let btn_w = if is_mobile {
                            (ui.available_width() - 12.0) / 2.0
                        } else {
                            140.0
                        };
                        let retry_btn = crate::widgets::ThemeButton::new(&strings.connection_retry)
                            .style(crate::widgets::ThemeButtonStyle::Primary)
                            .min_size(egui::vec2(btn_w, 40.0));
                        if ui.add(retry_btn).clicked() {
                            retry = true;
                        }
                        ui.add_space(12.0);
                        let dismiss_btn = crate::widgets::ThemeButton::new(&strings.dismiss)
                            .style(crate::widgets::ThemeButtonStyle::Danger)
                            .min_size(egui::vec2(btn_w, 40.0));
                        if ui.add(dismiss_btn).clicked() {
                            clear_error = true;
                        }
                    });

                    ui.add_space(4.0);
                });
            });

        if retry {
            state.error_message = None;
            action = Some(UiAction::RetryConnection);
        } else if clear_error {
            state.error_message = None;
        }
    }

    action
}
