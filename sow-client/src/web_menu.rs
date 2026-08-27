//! Web menu bridge.
//!
//! The browser/WebView owns presentation and input for the main menu. Rust remains the
//! source of truth for connection state, lobby state, identity, and match transitions.
//! Commands cross this boundary as small JSON messages; the existing UiAction and network
//! paths execute them unchanged.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
// std::time::Instant panics on wasm32-unknown-unknown ("time not implemented
// on this platform"); web_time::Instant wraps Performance.now() there instead.
use web_time::Instant;

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
    JoinWithPassword {
        lobby_id: u64,
        password: String,
    },
    JoinCode {
        code: String,
    },
    StartSinglePlayer {
        config: serde_json::Value,
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
    KickPlayer {
        lobby_id: u64,
        target_player_id: u16,
    },
    BanPlayer {
        lobby_id: u64,
        target_player_id: u16,
    },
    MovePlayerTeam {
        lobby_id: u64,
        target_player_id: u16,
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
    SetAttackRatio {
        ratio: f32,
    },
    SpawnTroops,
    BuildStructure {
        kind: String,
    },
    Surrender,
    ToggleLeaderboard,
    ReturnToMenu,
}

thread_local! {
    static COMMANDS: RefCell<VecDeque<WebMenuCommand>> = RefCell::new(VecDeque::new());
    /// Last payload handed to JS. publish_state runs every frame; without this
    /// guard each frame allocates a fresh JSON string plus a JS-side copy.
    static LAST_PUBLISHED: RefCell<String> = RefCell::new(String::new());
    /// Cheap fingerprint for the small, hot in-match HUD payload. This avoids rebuilding and
    /// serializing the payload when neither the displayed values nor the simulation snapshot
    /// changed.
    static LAST_HUD_KEY: RefCell<Option<HudPublishKey>> = const { RefCell::new(None) };
    /// The shell polls SOW_MENU_STATE at 80ms; publishing faster than that is
    /// invisible work, so attempts are throttled to match the consumer.
    static LAST_PUBLISH_ATTEMPT: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Minimum gap between state-publish attempts, matching the JS poll cadence.
const PUBLISH_MIN_INTERVAL_MS: u128 = 75;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HudPublishKey {
    gold: u64,
    troops: u64,
    troop_rate: u64,
    attack_ratio: u32,
    spawn_timer_tenths: i32,
    active_attacks: usize,
    settings_mute: bool,
    settings_music_volume: u32,
    settings_reduced_motion: bool,
    leaderboard_open: bool,
    snapshot_tick: u64,
}

fn hud_publish_key(
    app: &sow_ui::ClientApp,
    snapshot_tick: u64,
    leaderboard_open: bool,
) -> HudPublishKey {
    HudPublishKey {
        gold: app.hud_state.gold.to_bits(),
        troops: app.hud_state.troops.to_bits(),
        troop_rate: app.hud_state.troop_rate.to_bits(),
        attack_ratio: app.hud_state.attack_ratio.to_bits(),
        // The browser displays tenths; finer changes cannot change the rendered HUD.
        spawn_timer_tenths: app
            .hud_state
            .spawn_timer_secs
            .map(|secs| (secs.max(0.0) * 10.0).round() as i32)
            .unwrap_or(-1),
        active_attacks: app.hud_state.attacks.len(),
        settings_mute: app.settings_state.mute_all,
        settings_music_volume: app.settings_state.music_volume.to_bits(),
        settings_reduced_motion: app.settings_state.reduced_motion,
        leaderboard_open,
        snapshot_tick: if leaderboard_open { snapshot_tick } else { 0 },
    }
}

fn publish_due() -> bool {
    let now = Instant::now();
    let due = LAST_PUBLISH_ATTEMPT.with(|attempt| match attempt.get() {
        Some(last) => now.duration_since(last).as_millis() >= PUBLISH_MIN_INTERVAL_MS,
        None => true,
    });
    if due {
        LAST_PUBLISH_ATTEMPT.with(|attempt| attempt.set(Some(now)));
    }
    due
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
                    crate::analytics::track("menu_quick_match");
                    self.request_join(None, false, None, None);
                }
                WebMenuCommand::JoinLobby { lobby_id } => {
                    crate::analytics::track_with(
                        "menu_join_attempt",
                        serde_json::json!({ "source": "browser", "lobby_id": lobby_id }),
                    );
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::JoinLobby(lobby_id)),
                    );
                }
                WebMenuCommand::JoinWithPassword {
                    lobby_id,
                    password,
                } => {
                    crate::analytics::track_with(
                        "menu_password_join_attempt",
                        serde_json::json!({ "lobby_id": lobby_id }),
                    );
                    self.ui.app.main_menu_state.join_password_input = password;
                    self.ui.app.main_menu_state.join_password_for_lobby = Some(lobby_id);
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::JoinWithPassword(lobby_id)),
                    );
                }
                WebMenuCommand::JoinCode { code } => {
                    crate::analytics::track("menu_code_join_attempt");
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
                } => {
                    crate::analytics::track("menu_custom_create");
                    match serde_json::from_value::<sow_core::game_config::GameConfig>(config) {
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
                    }
                }
                WebMenuCommand::StartSinglePlayer { config } => {
                    crate::analytics::track("menu_single_player_start");
                    match serde_json::from_value::<sow_core::game_config::GameConfig>(config) {
                        Ok(config) => self.process_ui_actions(
                            &self.ui.egui_ctx.clone(),
                            Some(UiAction::StartSinglePlayer(Box::new(config))),
                        ),
                        Err(error) => {
                            log::warn!("[WEB MENU] invalid single-player config: {error}");
                            self.ui.app.main_menu_state.error_message =
                                Some("Invalid game configuration".to_string());
                        }
                    }
                }
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
                WebMenuCommand::KickPlayer {
                    lobby_id,
                    target_player_id,
                } => {
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::KickPlayer {
                            lobby_id,
                            target_player_id,
                        }),
                    );
                }
                WebMenuCommand::BanPlayer {
                    lobby_id,
                    target_player_id,
                } => {
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::BanPlayer {
                            lobby_id,
                            target_player_id,
                        }),
                    );
                }
                WebMenuCommand::MovePlayerTeam {
                    lobby_id,
                    target_player_id,
                } => {
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::MovePlayerTeam {
                            lobby_id,
                            target_player_id,
                        }),
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
                WebMenuCommand::SetAttackRatio { ratio } => {
                    self.ui.app.hud_state.attack_ratio = ratio.clamp(0.05, 1.0);
                }
                WebMenuCommand::SpawnTroops => {
                    if let Some(player_id) = self.sim.my_player_id {
                        if let Some(player) = self
                            .sim
                            .current_snapshot
                            .as_ref()
                            .and_then(|s| s.players.iter().find(|p| p.id == player_id))
                        {
                            let (cap_x, cap_y) = (player.centroid_x as u32, player.centroid_y as u32);
                            self.send_intent(sow_core::protocol::GameplayIntent::Spawn {
                                x: cap_x,
                                y: cap_y,
                            });
                        }
                    }
                }
                WebMenuCommand::BuildStructure { kind } => {
                    let structure_kind = match kind.to_lowercase().as_str() {
                        "city" => sow_core::game::BuildingKind::City,
                        "factory" => sow_core::game::BuildingKind::Factory,
                        "port" => sow_core::game::BuildingKind::Port,
                        "bunker" => sow_core::game::BuildingKind::Bunker,
                        _ => sow_core::game::BuildingKind::City,
                    };
                    self.ui.app.hud_state.selected_building_kind = Some(structure_kind);
                }
                WebMenuCommand::Surrender => {
                    self.send_intent(sow_core::protocol::GameplayIntent::Resign);
                }
                WebMenuCommand::ToggleLeaderboard => {
                    self.ui.show_leaderboard = !self.ui.show_leaderboard;
                }
                WebMenuCommand::ReturnToMenu => {
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::LeaveLobby),
                    );
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

fn build_hud_payload(app: &sow_ui::ClientApp, leaderboard_open: bool) -> serde_json::Value {
    if app.phase != sow_ui::ClientPhase::Playing {
        return serde_json::Value::Null;
    }
    let mut payload = serde_json::json!({
        "gold": app.hud_state.gold,
        "troops": app.hud_state.troops,
        "troop_rate": app.hud_state.troop_rate,
        "attack_ratio": app.hud_state.attack_ratio,
        "spawn_timer_secs": app.hud_state.spawn_timer_secs,
        "active_attacks_count": app.hud_state.attacks.len(),
    });
    // The rankings are a cold, on-demand view. Serializing hundreds of players while the
    // drawer is closed was the dominant avoidable cost in the web bridge.
    if leaderboard_open {
        let my_pid = app.hud_state.my_player_id;
        let leaderboard: Vec<serde_json::Value> = app
            .hud_state
            .players
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "name": &p.name,
                    "troops": p.troops,
                    "tile_count": p.tile_count,
                    "is_alive": p.alive,
                    "is_me": p.id == my_pid,
                    "leader": leader_id(p.leader),
                })
            })
            .collect();
        payload["leaderboard"] = serde_json::Value::Array(leaderboard);
    }
    payload
}

fn splash_job_name(job: &sow_ui::ui::loading_screen::SplashJob) -> &'static str {
    match job {
        sow_ui::ui::loading_screen::SplashJob::Boot => "Boot",
        sow_ui::ui::loading_screen::SplashJob::EnterGame => "EnterGame",
        sow_ui::ui::loading_screen::SplashJob::ExitGame => "ExitGame",
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
    snapshot_tick: u64,
    leaderboard_open: bool,
) {
    if !publish_due() {
        return;
    }

    let payload = if app.phase == sow_ui_kit::ClientPhase::Playing {
        let hud_key = hud_publish_key(app, snapshot_tick, leaderboard_open);
        let hud_changed = LAST_HUD_KEY.with(|last| {
            let mut last = last.borrow_mut();
            if *last == Some(hud_key) {
                false
            } else {
                *last = Some(hud_key);
                true
            }
        });
        if !hud_changed {
            return;
        }
        serde_json::json!({
            "phase": "Playing",
            "hud": build_hud_payload(app, leaderboard_open),
            "settings": {
                "mute_all": app.settings_state.mute_all,
                "music_volume": app.settings_state.music_volume,
                "reduced_motion": app.settings_state.reduced_motion,
            },
        })
    } else {
        LAST_HUD_KEY.with(|last| *last.borrow_mut() = None);
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

        serde_json::json!({
            "phase": phase_name(app.phase),
            "loader_job": splash_job_name(&app.splash_state.job),
            "loader_progress": app.splash_state.progress.clamp(0.0, 1.0),
            "loader_status": app
                .splash_state
                .status_override
                .as_deref()
                .unwrap_or(app.splash_state.status_text.as_str()),
            "loader_done": app.splash_state.done,
            "connected": state.is_connected,
            "connecting": state.is_connecting,
            "waiting": state.is_waiting,
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
            "my_player_id": state.my_player_id,
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
            "custom_game_is_sp": state.custom_game_is_sp,
            "hud": serde_json::Value::Null,
            "settings": {
                "mute_all": app.settings_state.mute_all,
                "music_volume": app.settings_state.music_volume,
                "reduced_motion": app.settings_state.reduced_motion,
            },
        })
    };

    let Ok(serialized) = serde_json::to_string(&payload) else {
        log::warn!("[WEB MENU] failed to serialize state");
        return;
    };

    let unchanged = LAST_PUBLISHED.with(|last| last.borrow().as_str() == serialized.as_str());
    if unchanged {
        return;
    }
    LAST_PUBLISHED.with(|last| *last.borrow_mut() = serialized.clone());

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
