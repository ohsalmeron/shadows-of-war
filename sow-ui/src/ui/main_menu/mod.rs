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
}

impl Default for MainMenuState {
    fn default() -> Self {
        Self {
            is_connected: false,
            is_connecting: false,
            is_waiting: false,
            wait_timer_secs: 0.0,
            server_address: std::env::var("SOW_WS_URL")
                .unwrap_or_else(|_| "ws://127.0.0.1:25565".to_string()),
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
                match ms % 11 {
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
                    _ => sow_core::player::Leader::Leonidas,
                }
            },
            selected_civilization: {
                let ms = web_time::SystemTime::now()
                    .duration_since(web_time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                match ms % 11 {
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
                    _ => sow_core::player::Civilization::Sparta,
                }
            },
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
                map_name: "World".to_string(),
                seed: web_time::SystemTime::now()
                    .duration_since(web_time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                ..Default::default()
            }),


            error_message: None,
        }
    }
}

#[inline]
pub fn lobby_compact_layout(ctx: &egui::Context) -> bool {
    ctx.content_rect().width() < 900.0 || ctx.content_rect().height() < 600.0
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

#[allow(clippy::too_many_arguments)]
fn draw_menu_right_panel_contents(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    section_gap: f32,
    action_min_h: f32,
    compact: bool,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_lang::Language,
) {
    let strings = &sow_lang::get(lang).main_menu;
    let version = format!("v{}", include_str!("../../../../.version").trim());

    profile::draw_user_profile_header(ui, state, compact, asset_loader, lang);

    if !state.is_connected {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(
                egui::RichText::new(&strings.connecting)
                    .color(crate::ui::theme::text_secondary()),
            );
        });
    }

    ui.add_space(section_gap);
    browser::draw_left_column(
        ui,
        state,
        section_gap,
        action_min_h,
        compact,
        action,
        asset_loader,
        lang,
    );

    ui.add_space(section_gap);
    actions::draw_right_column(
        ui,
        state,
        section_gap,
        action_min_h,
        compact,
        action,
        lang,
    );

    ui.add_space(section_gap);
    ui.label(
        egui::RichText::new(version)
            .size(12.0)
            .color(crate::ui::theme::text_secondary()),
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_menu_right_panel(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    section_gap: f32,
    action_min_h: f32,
    compact: bool,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_lang::Language,
) {
    if compact {
        draw_menu_right_panel_contents(
            ui,
            state,
            section_gap,
            action_min_h,
            compact,
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
                action,
                asset_loader,
                lang,
            );
        });
    }
}

pub fn draw(
    root_ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    lang: sow_lang::Language,
) -> Option<UiAction> {
    let mut action = None;
    let compact = lobby_compact_layout(root_ui.ctx());
    asset_loader.request_leader_portrait(state.selected_leader, compact);
    let outer_pad = 16.0;
    let section_gap = if compact { 12.0 } else { 16.0 };

    let action_min_h = if compact { 64.0 } else { 72.0 };
    let strings = &sow_lang::get(lang).main_menu;

    CentralPanel::default()
        .frame(
            Frame::new()
                .fill(Color32::TRANSPARENT)
                .inner_margin(outer_pad),
        )
        .show_inside(root_ui, |ui| {
            // Draw high-fidelity selected leader background texture
            let screen_rect = ui.ctx().content_rect();
            let is_mobile = compact;

            let background_tex = if is_mobile {
                asset_loader
                    .leader_mobile_images
                    .get(&state.selected_leader)
            } else {
                asset_loader
                    .leader_desktop_images
                    .get(&state.selected_leader)
            };

            if let Some(texture) = background_tex {
                let uv = crate::widgets::avatar_picker::calculate_cover_uv(
                    screen_rect.size(),
                    texture.size_vec2(),
                );
                ui.painter().image(
                    texture.id(),
                    screen_rect,
                    uv,
                    Color32::WHITE,
                );
            } else {
                ui.painter().rect_filled(
                    screen_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(10, 10, 15, 255),
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

            if compact {
                let viewport = ui.available_size();
                ui.allocate_ui_with_layout(viewport, Layout::top_down(Align::Min), |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_min_width(viewport.x);
                            draw_menu_right_panel(
                                ui,
                                state,
                                section_gap,
                                action_min_h,
                                compact,
                                &mut action,
                                asset_loader,
                                lang,
                            );
                        });
                });
            } else {
                ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                    let total = ui.available_width();
                    let panel_w = (total / 3.0).clamp(340.0, 460.0);
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
                                &mut action,
                                asset_loader,
                                lang,
                            );
                        },
                    );
                });
            }
        });

    if state.show_leader_picker
        && crate::widgets::draw_leader_picker_modal(
            root_ui.ctx(),
            &mut state.selected_leader,
            &mut state.selected_civilization,
            asset_loader,
        )
    {
        state.show_leader_picker = false;
    }

    if state.show_single_player_setup {
        single_player_setup::draw_modal(
            root_ui.ctx(),
            state,
            asset_loader,
            &mut action,
            lang,
            compact,
        );
    }

    if let Some(err_msg) = &state.error_message {
        let mut clear_error = false;

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
            .fixed_size(egui::vec2(modal_w, 240.0))
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

                    // Center-aligned, beautifully colored error details in uppercase
                    ui.label(
                        egui::RichText::new(err_msg.to_uppercase())
                            .size(14.0)
                            .color(crate::ui::theme::text_secondary())
                            .strong(),
                    );

                    ui.add_space(24.0);

                    // Premium dismissal button
                    let btn_w = if is_mobile {
                        ui.available_width()
                    } else {
                        160.0
                    };
                    let dismiss_btn = crate::widgets::ThemeButton::new(&strings.dismiss)
                        .style(crate::widgets::ThemeButtonStyle::Danger)
                        .min_size(egui::vec2(btn_w, 40.0));

                    if ui.add(dismiss_btn).clicked() {
                        clear_error = true;
                    }

                    ui.add_space(4.0);
                });
            });

        if clear_error {
            state.error_message = None;
        }
    }

    action
}
