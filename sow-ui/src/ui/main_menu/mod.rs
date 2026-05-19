pub mod actions;
pub mod browser;
pub mod profile;
pub mod queue_overlay;
pub mod single_player_setup;

use crate::UiAction;
use egui::{Align, CentralPanel, Color32, CornerRadius, Frame, Layout};
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
    pub cached_manifest: Option<sow_core::map_legacy::MapManifest>,
    pub map_download_progress: u8,
    pub show_avatar_picker: bool,
    pub selected_avatar_id: u8,
    pub show_single_player_setup: bool,
    pub single_player_config: Box<sow_core::game_config::GameConfig>,
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
            pending_join_lobby_id: None,
            joined_lobby_id: None,
            downloading_map_name: None,
            is_downloading_map: false,
            cached_map: None,
            cached_manifest: None,
            map_download_progress: 0,
            show_avatar_picker: false,
            selected_avatar_id: 255,
            show_single_player_setup: false,
            single_player_config: Box::new(sow_core::game_config::GameConfig::default()),
        }
    }
}

#[inline]
pub fn lobby_compact_layout(ctx: &egui::Context) -> bool {
    ctx.content_rect().width() < 900.0
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

pub fn draw(
    root_ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
) -> Option<UiAction> {
    let mut action = None;
    let compact = lobby_compact_layout(root_ui.ctx());
    let outer_pad = if compact { 16.0 } else { 24.0 };
    let section_gap = if compact { 12.0 } else { 16.0 };

    let status_large = if compact { 28.0 } else { 40.0 };
    let action_min_h = if compact { 64.0 } else { 72.0 };

    CentralPanel::default()
        .frame(
            Frame::new()
                .fill(crate::ui::theme::menu_backdrop())
                .inner_margin(outer_pad),
        )
        .show_inside(root_ui, |ui| {
            if state.is_waiting {
                queue_overlay::draw_queue_overlay(
                    ui,
                    state,
                    status_large,
                    section_gap,
                    action_min_h,
                    &mut action,
                );
                return;
            }

            let panel_frame = Frame::new()
                .fill(crate::ui::theme::panel_bg())
                .stroke(egui::Stroke::new(1.0_f32, crate::ui::theme::menu_panel_border_glow()))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(if compact { 18.0 } else { 24.0 })
                .shadow(egui::Shadow {
                    blur: 24,
                    spread: 0,
                    color: Color32::from_rgba_unmultiplied(6, 182, 212, 30),
                    offset: [0, 10],
                });

            panel_frame.show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            if !compact {
                                profile::draw_user_profile_header(ui, state, compact, asset_loader);
                            }
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if !state.is_connected {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(egui::RichText::new("Connecting…").color(crate::ui::theme::text_secondary()));
                                });
                            }
                        });
                    });

                    if compact {
                        ui.add_space(8.0);
                        profile::draw_user_profile_header(ui, state, compact, asset_loader);
                        ui.add_space(8.0);

                        ui.vertical(|ui| {
                            browser::draw_left_column(
                                ui,
                                state,
                                section_gap,
                                action_min_h,
                                compact,
                                &mut action,
                                asset_loader,
                            );
                            ui.add_space(section_gap);
                            actions::draw_right_column(ui, state, section_gap, action_min_h, compact, &mut action);
                        });
                    } else {
                        ui.horizontal_top(|ui| {
                            let total = ui.available_width();
                            let gap = section_gap;
                            let left_w = (total - gap) * 0.58;
                            let right_w = (total - gap) * 0.34;

                            ui.allocate_ui_with_layout(
                                egui::vec2(left_w, ui.available_height()),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    browser::draw_left_column(
                                        ui,
                                        state,
                                        section_gap,
                                        action_min_h,
                                        compact,
                                        &mut action,
                                        asset_loader,
                                    );
                                },
                            );

                            ui.allocate_ui_with_layout(
                                egui::vec2(right_w.clamp(280.0, 420.0), ui.available_height()),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    actions::draw_right_column(
                                        ui,
                                        state,
                                        section_gap,
                                        action_min_h,
                                        compact,
                                        &mut action,
                                    );
                                },
                            );
                        });
                    }

                    ui.add_space(section_gap);
                    ui.separator();
                    ui.add_space(6.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "v{} — Shadows of War",
                                env!("CARGO_PKG_VERSION")
                            ))
                            .small()
                            .color(crate::ui::theme::text_secondary()),
                        );
                    });
                });
            });
        });

    if state.show_avatar_picker {
        if crate::widgets::draw_avatar_picker_modal(root_ui.ctx(), &mut state.selected_avatar_id, asset_loader) {
            state.show_avatar_picker = false;
        }
    }

    if state.show_single_player_setup {
        single_player_setup::draw_modal(root_ui.ctx(), state, &mut action);
    }

    action
}
