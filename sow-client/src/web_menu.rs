//! Web menu bridge.
//!
//! The browser/WebView owns presentation and input for the main menu. Rust remains the
//! source of truth for connection state, lobby state, identity, and match transitions.
//! Commands cross this boundary as small JSON messages; the existing UiAction and network
//! paths execute them unchanged.

use std::cell::RefCell;
use std::collections::VecDeque;

use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::app::SowApp;
use sow_ui::UiAction;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WebMenuCommand {
    QuickMatch,
    JoinLobby {
        lobby_id: u64,
    },
    JoinCode {
        code: String,
    },
    CreateGame {
        config: serde_json::Value,
        #[serde(default)]
        is_private: bool,
        #[serde(default)]
        password: Option<String>,
    },
    SetLeader {
        leader_id: String,
    },
    SaveDisplayName {
        name: String,
    },
    OpenBrowser,
    OpenCreate,
    CloseOverlay,
    LeaveLobby,
    StartPrivate {
        lobby_id: u64,
    },
    SignIn,
    SetMute {
        value: bool,
    },
    SetMusicVolume {
        value: f32,
    },
    SetReducedMotion {
        value: bool,
    },
}

thread_local! {
    static COMMANDS: RefCell<VecDeque<WebMenuCommand>> = RefCell::new(VecDeque::new());
}

/// Called by the vanilla shell after the WASM module has initialized.
#[wasm_bindgen]
pub fn sow_menu_command(json: String) {
    match serde_json::from_str::<WebMenuCommand>(&json) {
        Ok(command) => COMMANDS.with(|commands| commands.borrow_mut().push_back(command)),
        Err(error) => log::warn!("[WEB MENU] invalid command: {error}"),
    }
}

fn take_commands() -> Vec<WebMenuCommand> {
    COMMANDS.with(|commands| commands.borrow_mut().drain(..).collect())
}

impl SowApp {
    pub(crate) fn process_web_menu_commands(&mut self) {
        for command in take_commands() {
            match command {
                WebMenuCommand::QuickMatch => {
                    self.request_join(None, false, None, None);
                }
                WebMenuCommand::JoinLobby { lobby_id } => {
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::JoinLobby(lobby_id)),
                    );
                }
                WebMenuCommand::JoinCode { code } => {
                    self.ui.app.main_menu_state.join_lobby_code = code;
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::JoinWithCode),
                    );
                }
                WebMenuCommand::CreateGame {
                    config,
                    is_private,
                    password,
                } => match serde_json::from_value::<sow_core::game_config::GameConfig>(config) {
                    Ok(config) => self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::CreateGame {
                            config: Box::new(config),
                            is_private,
                            password,
                        }),
                    ),
                    Err(error) => {
                        log::warn!("[WEB MENU] invalid create-game config: {error}");
                        self.ui.app.main_menu_state.error_message =
                            Some("Invalid game configuration".to_string());
                    }
                },
                WebMenuCommand::SetLeader { leader_id } => {
                    let value = serde_json::Value::String(leader_id);
                    match serde_json::from_value::<sow_core::player::Leader>(value) {
                        Ok(leader) => {
                            self.ui.app.main_menu_state.selected_leader = leader;
                            self.ui.app.main_menu_state.selected_civilization = leader.civilization();
                            self.ui.app.main_menu_state.custom_game_config.player_leader = leader;
                            self.ui.app.main_menu_state.custom_game_config.player_civilization =
                                leader.civilization();
                        }
                        Err(error) => log::warn!("[WEB MENU] invalid leader: {error}"),
                    }
                }
                WebMenuCommand::SaveDisplayName { name } => {
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::SaveDisplayName(name)),
                    );
                }
                WebMenuCommand::OpenBrowser => {
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::OpenJoinBrowser),
                    );
                }
                WebMenuCommand::OpenCreate => {
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::OpenCreateGame),
                    );
                }
                WebMenuCommand::CloseOverlay => {
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::CloseOverlay),
                    );
                }
                WebMenuCommand::LeaveLobby => {
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::LeaveLobby),
                    );
                }
                WebMenuCommand::StartPrivate { lobby_id } => {
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::StartPrivateLobby(lobby_id)),
                    );
                }
                WebMenuCommand::SignIn => {
                    crate::store_portals::show_auth_prompt();
                }
                WebMenuCommand::SetMute { value } => {
                    self.ui.app.settings_state.mute_all = value;
                }
                WebMenuCommand::SetMusicVolume { value } => {
                    self.ui.app.settings_state.music_volume = value.clamp(0.0, 1.0);
                }
                WebMenuCommand::SetReducedMotion { value } => {
                    self.ui.app.settings_state.reduced_motion = value;
                }
            }
        }
    }
}

fn phase_name(phase: sow_ui::ClientPhase) -> &'static str {
    match phase {
        sow_ui::ClientPhase::Splash => "Splash",
        sow_ui::ClientPhase::MainMenu => "MainMenu",
        sow_ui::ClientPhase::Playing => "Playing",
    }
}

fn leader_id(leader: sow_core::player::Leader) -> String {
    serde_json::to_value(leader)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| leader.name().replace(' ', ""))
}

fn notice_name(notice: Option<sow_ui::LobbyNotice>) -> Option<&'static str> {
    match notice {
        Some(sow_ui::LobbyNotice::HostLeft) => Some("host_left"),
        Some(sow_ui::LobbyNotice::Kicked) => Some("kicked"),
        Some(sow_ui::LobbyNotice::Banned) => Some("banned"),
        Some(sow_ui::LobbyNotice::ConnectionLost) => Some("connection_lost"),
        None => None,
    }
}

/// Publish a browser-safe snapshot. It is intentionally separate from MainMenuState so the
/// DOM never receives transient textures, map bytes, or internal auth/session material.
pub(crate) fn publish_state(
    state: &sow_ui::ui::main_menu::MainMenuState,
    app: &sow_ui::ClientApp,
    progress: &crate::player_progress::PlayerProgress,
) {
    let leaders: Vec<serde_json::Value> = sow_core::player::Leader::ALL
        .into_iter()
        .map(|leader| {
            serde_json::json!({
                "id": leader_id(leader),
                "name": leader.name(),
                "civilization": leader.civilization().name(),
                "perk": leader.perk_description(),
                "slug": leader.name().to_lowercase().replace(' ', "_"),
            })
        })
        .collect();
    let map_catalog: Vec<serde_json::Value> = app
        .asset_loader
        .map_catalog
        .as_ref()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| {
                    serde_json::json!({
                        "key": &entry.key,
                        "display_name": &entry.display_name,
                        "width": entry.width,
                        "height": entry.height,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let payload = serde_json::json!({
        "phase": phase_name(app.phase),
        "connected": state.is_connected,
        "connecting": state.is_connecting,
        "waiting": state.is_waiting,
        "wait_timer_secs": state.wait_timer_secs,
        "player_name": state.player_name,
        "name_locked": state.name_locked,
        "selected_leader": leader_id(state.selected_leader),
        "selected_leader_name": state.selected_leader.name(),
        "selected_civilization": state.selected_civilization.name(),
        "show_browser": state.show_join_browser,
        "show_create": state.show_custom_game,
        "join_lobby_code": state.join_lobby_code,
        "joined_lobby_id": state.joined_lobby_id,
        "pending_lobby_id": state.pending_join_lobby_id,
        "is_lobby_host": state.is_lobby_host,
        "downloading_map": state.is_downloading_map,
        "map_download_progress": state.map_download_progress,
        "lobbies": state.lobbies,
        "error": state.error_message,
        "notice": notice_name(state.notice),
        "level": progress.level,
        "xp": progress.xp,
        "laurels": progress.laurels,
        "leaders": leaders,
        "map_catalog": map_catalog,
        "custom_game_config": &*state.custom_game_config,
        "custom_game_is_private": state.custom_game_is_private,
        "settings": {
            "mute_all": app.settings_state.mute_all,
            "music_volume": app.settings_state.music_volume,
            "reduced_motion": app.settings_state.reduced_motion,
        },
    });

    let Ok(serialized) = serde_json::to_string(&payload) else {
        log::warn!("[WEB MENU] failed to serialize state");
        return;
    };

    let Some(window) = web_sys::window() else {
        return;
    };
    if let Err(error) = js_sys::Reflect::set(
        window.as_ref(),
        &JsValue::from_str("SOW_MENU_STATE"),
        &JsValue::from_str(&serialized),
    ) {
        log::warn!("[WEB MENU] failed to publish state: {:?}", error);
    }
}
