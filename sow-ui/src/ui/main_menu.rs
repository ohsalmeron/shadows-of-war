//! Lobby browser layout aligned with Shadows of War (`dark-rift/crates/client/src/ui/lobby_browser.rs`):
//! header, two-column desktop / stacked compact, queue overlay, scrollable lobby cards.

use crate::UiAction;
use crate::ui::theme::{
    self, accent_danger, accent_danger_border, accent_ranked_gold, accent_ranked_gold_hover,
    accent_solo_cyan, accent_solo_cyan_hover, menu_backdrop, menu_panel_border_glow,
    menu_secondary_button, nickname_field_border, panel_bg, text_secondary,
};
use egui::{
    Align, CentralPanel, Color32, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea,
    Stroke,
};
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
    pub map_download_progress: u8,
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
                let ms = web_time::SystemTime::now().duration_since(web_time::SystemTime::UNIX_EPOCH).unwrap_or_default().as_millis();
                format!("ANON{:03}", ms % 1000)
            },
            pending_join_lobby_id: None,
            joined_lobby_id: None,
            downloading_map_name: None,
            is_downloading_map: false,
            cached_map: None,
            map_download_progress: 0,
        }
    }
}

#[inline]
fn lobby_compact_layout(ctx: &egui::Context) -> bool {
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

#[allow(deprecated)]
pub fn draw(ctx: &egui::Context, state: &mut MainMenuState, asset_loader: &crate::ui::asset_loader::AssetLoader) -> Option<UiAction> {
    let mut action = None;
    let compact = lobby_compact_layout(ctx);
    let outer_pad = if compact { 16.0 } else { 24.0 };
    let section_gap = if compact { 12.0 } else { 16.0 };
    let title_fs = if compact { 40.0 } else { 56.0 };
    let status_large = if compact { 28.0 } else { 40.0 };
    let action_min_h = if compact { 64.0 } else { 72.0 };

    CentralPanel::default()
        .frame(
            Frame::new()
                .fill(theme::menu_backdrop())
                .inner_margin(outer_pad),
        )
        .show(ctx, |ui| {
            if state.is_waiting {
                draw_queue_overlay(
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
                .fill(panel_bg())
                .stroke(Stroke::new(1.5_f32, menu_panel_border_glow()))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(if compact { 18.0 } else { 24.0 });

            panel_frame.show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new("SHADOWS OF WAR")
                                    .font(egui::FontId::proportional(title_fs))
                                    .color(Color32::WHITE),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if !state.is_connected {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(RichText::new("Connecting…").color(text_secondary()));
                                });
                            }
                        });
                    });

                    ui.add_space(section_gap);

                    if compact {
                        ui.vertical(|ui| {
                            ui.add_space(4.0);
                            draw_left_column(
                                ui,
                                state,
                                section_gap,
                                action_min_h,
                                compact,
                                &mut action,
                                asset_loader,
                            );
                            ui.add_space(section_gap);
                            draw_right_column(
                                ui,
                                section_gap,
                                action_min_h,
                                compact,
                                &mut action,
                            );
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
                                    draw_left_column(
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
                                    draw_right_column(
                                        ui,
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
                            RichText::new(format!(
                                "v{} — Shadows of War",
                                env!("CARGO_PKG_VERSION")
                            ))
                            .small()
                            .color(text_secondary()),
                        );
                    });
                });
            });
        });

    action
}

fn draw_queue_overlay(
    ui: &mut egui::Ui,
    state: &MainMenuState,
    status_large: f32,
    section_gap: f32,
    action_min_h: f32,
    action: &mut Option<UiAction>,
) {
    Frame::new()
        .fill(menu_backdrop())
        .inner_margin(outer_pad_for_overlay(ui.ctx()))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("WAITING FOR PLAYERS…")
                        .size(status_large)
                        .strong()
                        .color(Color32::WHITE),
                );
                ui.add_space(section_gap);
                ui.label(
                    RichText::new(format!("Starting in: {:.1}s", state.wait_timer_secs))
                        .size(20.0)
                        .color(Color32::from_rgb(255, 210, 120)),
                );
                ui.add_space(section_gap);
                
                if let Some(lobby_id) = state.joined_lobby_id.or(state.pending_join_lobby_id) {
                    if let Some(lobby) = state.lobbies.iter().find(|l| l.id == lobby_id) {
                        ui.label(RichText::new(format!("Connected Players ({}/{})", lobby.num_players, lobby.max_players)).strong().color(text_secondary()));
                        ui.add_space(8.0);
                        for p in &lobby.players {
                            Frame::new()
                                .fill(menu_secondary_button())
                                .stroke(Stroke::new(1.0_f32, nickname_field_border()))
                                .corner_radius(CornerRadius::same(6))
                                .inner_margin(Margin::same(10))
                                .show(ui, |ui| {
                                    ui.set_width(220.0);
                                    ui.horizontal(|ui| {
                                        let map_ready = p.download_progress == 100 || p.is_ready;
                                        if map_ready {
                                            ui.label(RichText::new("✔").color(Color32::GREEN));
                                        } else {
                                            ui.add(egui::Spinner::new().size(12.0));
                                        }
                                        ui.add_space(8.0);
                                        ui.label(RichText::new(&p.name).size(16.0).color(Color32::WHITE));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if !map_ready && p.download_progress > 0 {
                                                ui.label(RichText::new(format!("{}%", p.download_progress)).size(12.0).color(text_secondary()));
                                            }
                                        });
                                    });
                                });
                            ui.add_space(6.0);
                        }
                    }
                }

                ui.add_space(section_gap * 1.5);

                let cancel = egui::Button::new(RichText::new("CANCEL").color(Color32::WHITE))
                    .fill(accent_danger())
                    .stroke(Stroke::new(1.0_f32, accent_danger_border()))
                    .min_size(egui::vec2(200.0, action_min_h));
                if ui.add(cancel).clicked() {
                    *action = Some(UiAction::LeaveLobby);
                }
            });
        });
}

fn outer_pad_for_overlay(ctx: &egui::Context) -> f32 {
    if lobby_compact_layout(ctx) {
        16.0
    } else {
        24.0
    }
}

fn draw_left_column(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    _section_gap: f32,
    action_min_h: f32,
    compact: bool,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
) {
    ui.label(
        RichText::new("Lobby Browser")
            .size(if compact { 14.0 } else { 16.0 })
            .color(text_secondary()),
    );
    ui.add_space(6.0);

    let scroll_h = if compact { 260.0 } else { 320.0 };
    ScrollArea::vertical()
        .max_height(scroll_h)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if state.lobbies.is_empty() {
                ui.label(
                    RichText::new("Waiting for lobby data from server…")
                        .color(text_secondary()),
                );
            } else {
                let ffa_lobbies: Vec<_> = state.lobbies.iter().filter(|l| l.game_mode == "FFA").collect();
                let team_lobbies: Vec<_> = state.lobbies.iter().filter(|l| l.game_mode == "Teams").collect();

                if !ffa_lobbies.is_empty() {
                    ui.label(RichText::new("Free For All").strong().color(Color32::WHITE));
                    ui.add_space(4.0);
                    for lobby in ffa_lobbies {
                        lobby_card(ui, lobby, action_min_h, action, asset_loader);
                        ui.add_space(8.0);
                    }
                }

                if !team_lobbies.is_empty() {
                    ui.add_space(8.0);
                    ui.label(RichText::new("Team Matches").strong().color(Color32::WHITE));
                    ui.add_space(4.0);
                    for lobby in team_lobbies {
                        lobby_card(ui, lobby, action_min_h, action, asset_loader);
                        ui.add_space(8.0);
                    }
                }
            }
        });
}

fn lobby_card(
    ui: &mut egui::Ui,
    lobby: &LobbyInfo,
    _action_min_h: f32,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
) {
    let desired_size = egui::vec2(ui.available_width(), 160.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    
    let is_hovered = response.hovered();
    if is_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let stroke_color = if lobby.is_counting_down {
        if is_hovered { accent_solo_cyan_hover() } else { accent_solo_cyan() }
    } else {
        if is_hovered { text_secondary() } else { nickname_field_border() }
    };

    // 1. Draw Background Image
    if let Some(texture) = asset_loader.thumbnail(&lobby.map_name) {
        let tint = if is_hovered { Color32::WHITE } else { Color32::from_gray(200) };
        let image = egui::Image::new(texture)
            .fit_to_exact_size(rect.size())
            .corner_radius(CornerRadius::same(8))
            .tint(tint);
        ui.put(rect, image);
    } else {
        ui.painter().rect_filled(rect, 8.0, menu_secondary_button());
    }

    ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.5_f32, stroke_color), egui::StrokeKind::Inside);

    // 3. Top Row Overlay (Mode & Timer)
    let top_rect = rect.shrink(8.0);
    let mode_text = if lobby.game_mode == "FFA" { "FFA" } else { "TEAMS" };
    
    // Draw Mode Badge
    let mode_galley = ui.painter().layout_no_wrap(
        mode_text.to_string(),
        egui::FontId::proportional(14.0),
        Color32::WHITE,
    );
    let mode_badge_rect = egui::Rect::from_min_size(top_rect.min, mode_galley.size() + egui::vec2(12.0, 6.0));
    ui.painter().rect_filled(mode_badge_rect, 4.0, accent_solo_cyan());
    ui.painter().galley(mode_badge_rect.center() - mode_galley.size() / 2.0, mode_galley, Color32::WHITE);

    // Draw Timer
    let timer_text = if lobby.is_counting_down {
        format!("Starts in {:.0}s", lobby.timer_secs.max(0.0))
    } else {
        "WAITING".to_string()
    };
    let timer_color = if lobby.is_counting_down { Color32::from_rgb(255, 210, 120) } else { text_secondary() };
    let timer_galley = ui.painter().layout_no_wrap(
        timer_text,
        egui::FontId::proportional(14.0),
        timer_color,
    );
    let timer_badge_rect = egui::Rect::from_min_size(
        egui::pos2(top_rect.max.x - timer_galley.size().x - 12.0, top_rect.min.y),
        timer_galley.size() + egui::vec2(12.0, 6.0),
    );
    ui.painter().rect_filled(timer_badge_rect, 4.0, Color32::from_black_alpha(180));
    ui.painter().galley(timer_badge_rect.center() - timer_galley.size() / 2.0, timer_galley, timer_color);

    // 4. Bottom Bar (Map Name & Players)
    let bottom_height = 44.0;
    let bottom_rect = egui::Rect::from_min_max(
        egui::pos2(rect.min.x, rect.max.y - bottom_height),
        rect.max,
    );
    ui.painter().rect_filled(
        bottom_rect,
        CornerRadius { nw: 0, ne: 0, sw: 8, se: 8 },
        Color32::from_black_alpha(200),
    );

    // Map Name
    let map_galley = ui.painter().layout_no_wrap(
        lobby.map_name.to_uppercase(),
        egui::FontId::proportional(18.0),
        Color32::WHITE,
    );
    ui.painter().galley(
        egui::pos2(bottom_rect.min.x + 12.0, bottom_rect.min.y + (bottom_height - map_galley.size().y) / 2.0),
        map_galley,
        Color32::WHITE,
    );

    // Players
    let players_text = format!("{}/{}", lobby.num_players, lobby.max_players);
    let players_galley = ui.painter().layout_no_wrap(
        players_text,
        egui::FontId::proportional(14.0),
        Color32::WHITE,
    );
    let players_badge_rect = egui::Rect::from_min_size(
        egui::pos2(bottom_rect.max.x - players_galley.size().x - 16.0, bottom_rect.min.y - 12.0),
        players_galley.size() + egui::vec2(12.0, 6.0),
    );
    ui.painter().rect_filled(players_badge_rect, 4.0, Color32::from_black_alpha(220));
    ui.painter().galley(players_badge_rect.center() - players_galley.size() / 2.0, players_galley, Color32::WHITE);

    if response.clicked() {
        *action = Some(UiAction::JoinLobby(lobby.id));
    }
}

fn draw_right_column(
    ui: &mut egui::Ui,
    section_gap: f32,
    action_min_h: f32,
    compact: bool,
    action: &mut Option<UiAction>,
) {
    let solo_primary = if compact { 24.0 } else { 28.0 };



    let solo_btn = egui::Button::new(
        RichText::new("SINGLE PLAYER").size(solo_primary).strong().color(Color32::BLACK),
    )
    .fill(accent_solo_cyan())
    .stroke(Stroke::new(2.0_f32, accent_solo_cyan_hover()))
    .min_size(egui::vec2(ui.available_width(), action_min_h));

    if ui.add(solo_btn).clicked() {
        *action = Some(UiAction::StartSinglePlayer);
    }

    ui.add_space(section_gap);

    let ranked = egui::Button::new(
        RichText::new("RANKED MATCH")
            .size(18.0)
            .strong()
            .color(Color32::WHITE),
    )
    .fill(accent_ranked_gold())
    .stroke(Stroke::new(1.0_f32, accent_ranked_gold_hover()))
    .min_size(egui::vec2(ui.available_width(), (action_min_h - 10.0).max(60.0)));

    if ui.add(ranked).clicked() {
        log::info!("Ranked match (stub — not implemented)");
    }

    ui.add_space(section_gap * 0.75);

    stub_secondary(ui, "CREATE LOBBY", compact);
    ui.add_space(section_gap * 0.5);
    stub_secondary(ui, "JOIN LOBBY", compact);
    ui.add_space(section_gap * 0.5);
    
    let h = if compact { 48.0 } else { 52.0 };
    let btn = egui::Button::new(
        RichText::new("⚙  Settings")
            .size(if compact { 16.0 } else { 18.0 })
            .color(text_secondary()),
    )
    .fill(menu_secondary_button())
    .stroke(Stroke::new(1.0_f32, nickname_field_border()))
    .min_size(egui::vec2(ui.available_width(), h));

    if ui.add(btn).clicked() {
        *action = Some(UiAction::ToggleSettings);
    }
}

fn stub_secondary(ui: &mut egui::Ui, label: &str, compact: bool) {
    let h = if compact { 60.0 } else { 68.0 };
    let btn = egui::Button::new(RichText::new(label).size(if compact { 17.0 } else { 19.0 }))
        .fill(menu_secondary_button())
        .stroke(Stroke::new(1.0_f32, nickname_field_border()))
        .min_size(egui::vec2(ui.available_width(), h));

    let r = ui.add(btn);
    if r.clicked() {
        log::info!("Menu stub: {}", label);
    }
    if r.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}
