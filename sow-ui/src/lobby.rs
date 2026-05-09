use egui::{Context, Align, Layout, Color32, RichText, CentralPanel};
use sow_core::protocol::LobbyInfo;
use crate::UiAction;

pub struct LobbyState {
    pub is_connected: bool,
    pub is_waiting: bool,
    pub wait_timer_secs: f32,
    pub server_address: String,
    pub lobbies: Vec<LobbyInfo>,
}

impl Default for LobbyState {
    fn default() -> Self {
        Self {
            is_connected: false,
            is_waiting: false,
            wait_timer_secs: 0.0,
            server_address: "ws://127.0.0.1:25565".to_string(),
            lobbies: Vec::new(),
        }
    }
}

#[allow(deprecated)]
pub fn draw(ctx: &Context, state: &mut LobbyState) -> Option<UiAction> {
    let mut action = None;

    CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.heading(RichText::new("SHADOWS OF WAR").size(48.0).color(Color32::from_rgb(0, 210, 255)));
            ui.add_space(20.0);

            if !state.is_connected {
                ui.label("Not Connected");
                ui.horizontal(|ui| {
                    ui.label("Server Address:");
                    ui.text_edit_singleline(&mut state.server_address);
                });
                if ui.button("Connect").clicked() {
                    action = Some(UiAction::ConnectToServer(state.server_address.clone()));
                }
                ui.add_space(20.0);
                if ui.button("Start Single Player").clicked() {
                    action = Some(UiAction::StartSinglePlayer);
                }
            } else if state.is_waiting {
                ui.label(RichText::new("Waiting for next game to start...").size(24.0));
                ui.label(RichText::new(format!("Time remaining: {:.1}s", state.wait_timer_secs)).size(18.0).color(Color32::GOLD));
            } else {
                ui.label(RichText::new("Server Lobby Browser").size(24.0));
                ui.add_space(10.0);
                
                if state.lobbies.is_empty() {
                    ui.label("No active lobbies found.");
                } else {
                    for lobby in &state.lobbies {
                        ui.horizontal(|ui| {
                            ui.label(format!("Map: {}", lobby.map_name));
                            ui.label(format!("Players: {}/{}", lobby.num_players, lobby.max_players));
                            if ui.button("Join").clicked() {
                                action = Some(UiAction::JoinLobby(lobby.id));
                            }
                        });
                    }
                }
                
                ui.add_space(20.0);
                if ui.button("Create Lobby").clicked() {
                    action = Some(UiAction::CreateLobby);
                }
                if ui.button("Disconnect").clicked() {
                    action = Some(UiAction::LeaveLobby);
                }
            }
        });
    });

    action
}
