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
            single_player_config: Box::new({
                let mut config = sow_core::game_config::GameConfig::default();
                config.map_name = "World".to_string();
                config
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

pub fn draw(
    root_ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_lang::Language,
) -> Option<UiAction> {
    let mut action = None;
    let compact = lobby_compact_layout(root_ui.ctx());
    let outer_pad = if compact { 12.0 } else { 16.0 };
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
            // Draw high-fidelity natural loader background texture
            let screen_rect = ui.ctx().content_rect();
            let screen_w = screen_rect.width();
            let screen_h = screen_rect.height();
            let is_mobile = compact;

            let background_tex = if is_mobile {
                asset_loader.splash_mobile.as_ref()
            } else {
                asset_loader.splash_desktop.as_ref()
            };

            if let Some(texture) = background_tex {
                let tex_aspect = texture.size()[0] as f32 / texture.size()[1] as f32;
                let screen_aspect = screen_w / screen_h;

                let (mut u0, mut v0, mut u1, mut v1) = (0.0, 0.0, 1.0, 1.0);

                if tex_aspect > screen_aspect {
                    let crop_w = screen_aspect / tex_aspect;
                    u0 = (1.0 - crop_w) / 2.0;
                    u1 = 1.0 - u0;
                } else {
                    let crop_h = tex_aspect / screen_aspect;
                    v0 = (1.0 - crop_h) / 2.0;
                    v1 = 1.0 - v0;
                }

                ui.painter().image(
                    texture.id(),
                    screen_rect,
                    egui::Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1)),
                    Color32::WHITE,
                );
            } else {
                ui.painter().rect_filled(
                    screen_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(10, 10, 15, 255),
                );
            }

            // Draw a translucent overlay to make main menu text perfectly readable
            ui.painter().rect_filled(
                screen_rect,
                0.0,
                Color32::from_black_alpha(120),
            );
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
                ui.set_min_size(ui.available_size());
                let show_footer = ui.available_height() > 430.0;
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            if !compact {
                                profile::draw_user_profile_header(ui, state, compact, asset_loader, lang);
                            }
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if !state.is_connected {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(egui::RichText::new(&strings.connecting).color(crate::ui::theme::text_secondary()));
                                });
                            }
                        });
                    });

                    if compact {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.add_space(8.0);
                            profile::draw_user_profile_header(ui, state, compact, asset_loader, lang);
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
                                    lang,
                                );
                                ui.add_space(section_gap);
                                actions::draw_right_column(ui, state, section_gap, action_min_h, compact, &mut action, lang);
                            });
                        });
                    } else {
                        ui.horizontal_top(|ui| {
                            let total = ui.available_width();
                            let gap = section_gap;
                            let left_w = (total - gap) * 0.58;
                            let right_w = (total - gap) * 0.34;
                            let footer_offset = if show_footer { section_gap + 22.0 } else { 0.0 };
                            let content_h = ui.available_height() - footer_offset;

                            ui.allocate_ui_with_layout(
                                egui::vec2(left_w, content_h),
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
                                        lang,
                                    );
                                },
                            );

                            ui.allocate_ui_with_layout(
                                egui::vec2(right_w.clamp(280.0, 420.0), content_h),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    actions::draw_right_column(
                                        ui,
                                        state,
                                        section_gap,
                                        action_min_h,
                                        compact,
                                        &mut action,
                                        lang,
                                    );
                                },
                            );
                        });
                    }

                    if show_footer {
                        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(format!(
                                    "v{}",
                                    include_str!("../../../../.version").trim()
                                ))
                                .size(2.0)
                                .color(Color32::TRANSPARENT),
                            );
                            ui.separator();
                        });
                    }
                });
            });
        });

    if state.show_avatar_picker && crate::widgets::draw_avatar_picker_modal(root_ui.ctx(), &mut state.selected_avatar_id, asset_loader) {
        state.show_avatar_picker = false;
    }

    if state.show_single_player_setup {
        single_player_setup::draw_modal(root_ui.ctx(), state, asset_loader, &mut action, lang);
    }

    if let Some(err_msg) = &state.error_message {
        let mut clear_error = false;
        
        // 1. Draw a full-screen backdrop to dim the background and intercept clicks
        egui::Area::new(egui::Id::new("error_modal_backdrop"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(root_ui.ctx(), |ui| {
                let screen_rect = ui.ctx().content_rect();
                ui.painter().rect_filled(screen_rect, 0.0, crate::ui::theme::menu_backdrop());
            });
            
        // 2. Draw responsive centered window on top
        let screen_rect = root_ui.ctx().content_rect();
        let is_mobile = screen_rect.width() < 600.0;
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
                    .stroke(egui::Stroke::new(1.5_f32, crate::ui::theme::accent_danger_border()))
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
                    let btn_w = if is_mobile { ui.available_width() } else { 160.0 };
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
