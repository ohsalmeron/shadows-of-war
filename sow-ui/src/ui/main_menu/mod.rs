pub mod actions;
pub mod browser;
pub mod profile;
pub mod queue_overlay;
pub mod single_player_setup;

use crate::UiAction;
use egui::{Align, CentralPanel, Color32, Frame, Layout};
use sow_core::protocol::LobbyInfo;

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
}

impl Default for MainMenuState {
    fn default() -> Self {
        Self {
            is_connected: false,
            is_connecting: false,
            is_waiting: false,
            wait_timer_secs: 0.0,
            server_address: std::env::var("SOW_WS_URL")
                .unwrap_or_else(|_| "wss://shadowsofwar.io/ws/".to_string()),
            lobbies: Vec::new(),
            player_name: {
                let ms = web_time::SystemTime::now()
                    .duration_since(web_time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                format!("ANON{:03}", ms % 1000)
            },
            clan_tag: "".to_string(),
            selected_leader: {
                let ms = web_time::SystemTime::now()
                    .duration_since(web_time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                match ms % 12 {
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
                }
            },
            selected_civilization: {
                let ms = web_time::SystemTime::now()
                    .duration_since(web_time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                match ms % 12 {
                    0 => sow_core::player::Civilization::Rome,
                    1 => sow_core::player::Civilization::Egypt,
                    2 => sow_core::player::Civilization::Vikings,
                    3 => sow_core::player::Civilization::China,
                    4 => sow_core::player::Civilization::Macedon,
                    5 => sow_core::player::Civilization::Mongols,
                    6 => sow_core::player::Civilization::Angevin,
                    7 => sow_core::player::Civilization::Gallic,
                    8 => sow_core::player::Civilization::Iceni,
                    9 => sow_core::player::Civilization::Maya,
                    10 => sow_core::player::Civilization::Sparta,
                    _ => sow_core::player::Civilization::France,
                }
            },
            name_locked: false,
            host_private_pending: false,
            in_private_match: false,
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
                seed: web_time::SystemTime::now()
                    .duration_since(web_time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                ..Default::default()
            }),
            leader_backdrop: {
                let leader = match web_time::SystemTime::now()
                    .duration_since(web_time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    % 12
                {
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
                crate::widgets::LeaderBackdropTransition::new(leader)
            },


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
}

#[inline]
pub fn lobby_compact_layout(ctx: &egui::Context) -> bool {
    crate::ui::theme::compact_viewport(ctx)
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

fn menu_meta_footer_height(section_gap: f32) -> f32 {
    section_gap * 0.5 + 14.0 + 4.0 + 14.0
}

fn menu_footer_height(section_gap: f32, action_min_h: f32) -> f32 {
    let secondary_h = (action_min_h - 10.0).max(action_min_h * 0.875);
    let settings_h = action_min_h * 0.75;
    action_min_h
        + section_gap
        + secondary_h
        + section_gap
        + settings_h
        + menu_meta_footer_height(section_gap)
        + 6.0
}

fn draw_menu_footer(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    section_gap: f32,
    action_min_h: f32,
    compact: bool,
    action: &mut Option<UiAction>,
    lang: sow_i18n::Language,
    version: &str,
) {
    actions::draw_right_column(
        ui,
        state,
        section_gap,
        action_min_h,
        compact,
        action,
        lang,
    );

    ui.add_space(section_gap * 0.5);
    let strings = &sow_i18n::get(lang).main_menu;
    let credits = &sow_i18n::get(lang).credits;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(version)
                    .size(12.0)
                    .color(crate::ui::theme::text_secondary()),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(&credits.based_on_short)
                    .size(11.0)
                    .color(crate::ui::theme::text_secondary()),
            );
        });
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new(&strings.credits_link)
                        .size(12.0)
                        .color(crate::ui::theme::accent_solo_cyan()),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
            *action = Some(UiAction::ToggleCredits);
        }
        
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let text_color = crate::ui::theme::text_secondary();
            let link_color = crate::ui::theme::accent_solo_cyan();
            let size = 11.0;
            
            ui.label(egui::RichText::new(&strings.by_playing_you_agree).size(size).color(text_color));
            
            if ui.add(
                egui::Button::new(egui::RichText::new(&sow_i18n::get(lang).settings.terms_of_service).size(size).color(link_color))
                    .fill(egui::Color32::TRANSPARENT).stroke(egui::Stroke::NONE)
            ).clicked() {
                *action = Some(UiAction::ToggleTerms);
            }
            
            ui.label(egui::RichText::new(&strings.and_the).size(size).color(text_color));
            
            if ui.add(
                egui::Button::new(egui::RichText::new(&sow_i18n::get(lang).settings.privacy_policy).size(size).color(link_color))
                    .fill(egui::Color32::TRANSPARENT).stroke(egui::Stroke::NONE)
            ).clicked() {
                *action = Some(UiAction::TogglePrivacy);
            }
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn draw_menu_right_panel_contents(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    section_gap: f32,
    action_min_h: f32,
    compact: bool,
    profile_height: f32,
    panel_inner_h: f32,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
) {
    let version = format!("v{}", include_str!("../../../../.version").trim());
    let footer_h = menu_footer_height(section_gap, action_min_h);
    let header_h = profile_height + section_gap;
    // Cap lobby thumbnail on short viewports only — never allocate a flex middle band.
    let max_lobby_h =
        (panel_inner_h - header_h - section_gap - footer_h).max(0.0);
    let panel_w = ui.available_width();

    let panel_rect = egui::Rect::from_min_size(
        ui.cursor().min,
        egui::vec2(panel_w, panel_inner_h),
    );

    ui.scope_builder(egui::UiBuilder::new().max_rect(panel_rect), |ui| {
        profile::draw_user_profile_header(
            ui,
            state,
            compact,
            profile_height,
            asset_loader,
            lang,
        );

        ui.add_space(section_gap);

        browser::draw_left_column(
            ui,
            state,
            section_gap,
            action_min_h,
            compact,
            max_lobby_h,
            action,
            asset_loader,
            lang,
        );

        ui.add_space(section_gap);

        draw_menu_footer(
            ui,
            state,
            section_gap,
            action_min_h,
            compact,
            action,
            lang,
            &version,
        );
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
                    panel_outer_h,
                    action,
                    asset_loader,
                    lang,
                );
            } else {
                crate::ui::theme::menu_right_panel_frame(false).show(ui, |ui| {
                    draw_menu_right_panel_contents(
                        ui,
                        state,
                        section_gap,
                        action_min_h,
                        compact,
                        profile_height,
                        ui.available_height(),
                        action,
                        asset_loader,
                        lang,
                    );
                });
            }
        },
    );
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
    let map_name = state
        .downloading_map_name
        .as_deref()
        .unwrap_or("map");
    let strings = &sow_i18n::get(lang).main_menu;
    let label = strings
        .downloading_map
        .replacen("{}", map_name, 1)
        .replacen("{}", &state.map_download_progress.to_string(), 1);
    let pad_x = if compact { 24.0 } else { 20.0 };
    let pad_y = if compact { 96.0 } else { 56.0 };

    egui::Area::new(egui::Id::new("main_menu_map_download"))
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
                        ui.label(
                            egui::RichText::new(label)
                                .color(crate::ui::theme::accent_solo_cyan())
                                .size(if compact { 13.0 } else { 14.0 }),
                        );
                    });
                });
        });
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
    let pad_x = if compact { 24.0 } else { 20.0 };
    let pad_y = if compact { 56.0 } else { 20.0 };

    egui::Area::new(egui::Id::new("main_menu_connecting"))
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
                        ui.label(
                            egui::RichText::new(&strings.connecting)
                                .color(crate::ui::theme::text_secondary())
                                .size(if compact { 13.0 } else { 14.0 }),
                        );
                    });
                });
        });
}

pub fn draw(
    root_ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
) -> Option<UiAction> {
    let mut action = None;
    let compact = lobby_compact_layout(root_ui.ctx());
    let scale = crate::ui::theme::viewport_scale(root_ui.ctx());
    let outer_pad = 16.0;
    let section_gap = (if compact { 12.0 } else { 16.0 }) * scale;
    let action_min_h = (if compact { 64.0 } else { 72.0 }) * scale;
    let profile_height = 56.0 * scale;
    let strings = &sow_i18n::get(lang).main_menu;

    if state.show_single_player_setup {
        single_player_setup::draw(root_ui, state, asset_loader, &mut action, lang);
    } else {
    CentralPanel::default()
        .frame(
            Frame::new()
                .fill(Color32::TRANSPARENT)
                .inner_margin(outer_pad),
        )
        .show_inside(root_ui, |ui| {
            let screen_rect = ui.ctx().content_rect();
            let is_mobile = compact;
            if !state.show_leader_picker {
                crate::widgets::draw_leader_hero_backdrop(
                    ui,
                    screen_rect,
                    state.selected_leader,
                    is_mobile,
                    asset_loader,
                    &mut state.leader_backdrop,
                    &strings.loading_leader_portrait,
                    false,
                );
            }

            if state.is_waiting {
                queue_overlay::draw_queue_overlay(
                    ui,
                    state,
                    section_gap,
                    action_min_h,
                    &mut action,
                    asset_loader,
                    lang,
                );
                return;
            }

            let content_h = ui.available_height();
            let panel_w =
                crate::ui::theme::menu_rail_panel_width(ui.available_width(), compact);

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
                        content_h,
                        &mut action,
                        asset_loader,
                        lang,
                    );
                },
            );
        });
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
                    .rect_filled(screen_rect, 0.0, crate::ui::theme::menu_backdrop());
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
                    .fill(crate::ui::theme::panel_bg())
                    .stroke(egui::Stroke::new(
                        1.5_f32,
                        crate::ui::theme::accent_danger_border(),
                    ))
                    .corner_radius(egui::CornerRadius::same(16))
                    .inner_margin(24.0)
                    .shadow(egui::Shadow {
                        blur: 32,
                        spread: 0,
                        color: crate::ui::theme::accent_danger().linear_multiply(0.2),
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
                        crate::ui::theme::accent_danger(),
                    );

                    ui.add_space(12.0);

                    // Outlined Game-Themed Title
                    crate::ui::theme::outlined_label(
                        ui,
                        &strings.connection_error_header,
                        egui::FontId::proportional(22.0),
                        crate::ui::theme::accent_danger(),
                    );

                    ui.add_space(16.0);

                    ui.label(
                        egui::RichText::new(err_msg.as_str())
                            .size(14.0)
                            .color(crate::ui::theme::text_secondary()),
                    );

                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(&strings.connection_error_hint)
                            .size(12.0)
                            .color(crate::ui::theme::text_secondary()),
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
