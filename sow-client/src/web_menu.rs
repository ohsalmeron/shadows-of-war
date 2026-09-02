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
use wasm_bindgen::JsCast;

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
    CancelPlacement,
    ToggleInbox,
    AcceptAlliance {
        target_player_id: u16,
    },
    RejectAlliance {
        target_player_id: u16,
    },
    OpenTransfer {
        target_player_id: u16,
    },
    CloseTransfer,
    SendResources {
        target_player_id: u16,
        gold: f64,
        troops: f64,
    },
    RequestResources {
        target_player_id: u16,
        gold: f64,
        troops: f64,
    },
    AcceptResourceRequest {
        target_player_id: u16,
    },
    RejectResourceRequest {
        target_player_id: u16,
    },
    ConfirmBetrayal,
    CancelBetrayal,
    CancelAttack {
        attack_id: u64,
    },
    RecallFleet {
        fleet_id: u64,
    },
    Surrender,
    ToggleLeaderboard,
    ToggleTutorialObjectives,
    ToggleDevSidebar,
    SetDevConfig {
        field: WebDevConfigField,
        value: f32,
    },
    ResetDevConfig,
    ReturnToMenu,
    ContinueObserving,
    ZoomIn,
    ZoomOut,
    CenterCamera,
    ExpressEmoji {
        emoji: String,
        #[serde(default)]
        pinned: bool,
    },
    SetEmojiPinned {
        pinned: bool,
    },
    FocusPlayer {
        player_id: u16,
    },
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WebDevConfigField {
    Thickness,
    Darkness,
    ShoreThickness,
    ConquestDuration,
    TerritoryOpacity,
}

thread_local! {
    static COMMANDS: RefCell<VecDeque<WebMenuCommand>> = RefCell::new(VecDeque::new());
    /// Last payload handed to JS. publish_state runs every frame; without this
    /// guard each frame allocates a fresh JSON string plus a JS-side copy.
    static LAST_PUBLISHED: RefCell<String> = RefCell::new(String::new());
    /// Cheap fingerprint for the small, hot in-match HUD payload. Heavy cold panels are
    /// represented by the snapshot tick only while a panel that needs them is open.
    static LAST_HUD_KEY: RefCell<Option<HudPublishKey>> = const { RefCell::new(None) };
    /// The browser consumes changed payloads through SOW_onStateUpdate. The
    /// fallback menu poll does not run during gameplay, so this keeps the
    /// bridge from attempting work more often than the visible HUD can change.
    static LAST_PUBLISH_ATTEMPT: Cell<Option<Instant>> = const { Cell::new(None) };
    /// Player-derived hot values are cached by snapshot tick so the player list is not scanned
    /// on every publish attempt.
    static LAST_MY_PLAYER: RefCell<Option<(u64, u16, Option<MyPlayerSummary>)>> =
        const { RefCell::new(None) };
    /// WASM nameplates need the same rank cache as native egui for the top-three crown/medals.
    /// Refreshing is keyed by the authoritative snapshot tick, not the render/publish cadence.
    static LAST_WEB_RANKINGS_TICK: Cell<Option<u64>> = const { Cell::new(None) };
    /// Cold panel JSON is rebuilt only when its snapshot input changes. Hot HUD updates reuse it.
    static LAST_WEB_LEADERBOARD_PAYLOAD:
        RefCell<Option<(u64, u16, serde_json::Value)>> = const { RefCell::new(None) };
    static LAST_WEB_HOVER_PAYLOAD:
        RefCell<Option<(u64, u32, u16, u16, serde_json::Value)>> = const { RefCell::new(None) };
    static LAST_WEB_INBOX_PAYLOAD:
        RefCell<Option<(u64, u16, serde_json::Value)>> = const { RefCell::new(None) };
}

/// Minimum gap between state-publish attempts, matching the JS poll cadence.
const PUBLISH_MIN_INTERVAL_MS: u128 = 75;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HudPublishKey {
    gold: u64,
    troops: u64,
    max_troops: u64,
    troop_rate: u64,
    attack_ratio: u32,
    spawn_timer_tenths: i32,
    selected_building: u8,
    selected_nuke: u8,
    building_costs: [u64; 9],
    settings_mute: bool,
    settings_music_volume: u32,
    settings_reduced_motion: bool,
    leaderboard_open: bool,
    tutorial_active: bool,
    tutorial_objectives_open: bool,
    dev_sidebar_open: bool,
    dev_thickness: u32,
    dev_darkness: u32,
    dev_shore_thickness: u32,
    dev_conquest_duration: u32,
    dev_territory_opacity: u32,
    inbox_open: bool,
    transfer_target: Option<u16>,
    betrayal_open: bool,
    sync_open: bool,
    inbox_count: usize,
    notification_len: usize,
    is_spectating: bool,
    snapshot_tick: u64,
    hovered_tile: u32,
    hovered_owner: u16,
}

#[derive(Clone, Copy)]
struct MyPlayerSummary {
    gold: f64,
    troops: f64,
    max_troops: f64,
    alive: bool,
    has_spawned: bool,
    team: Option<sow_core::protocol::Team>,
    leader: sow_core::player::Leader,
    kills: u32,
    deaths: u32,
    assists: u32,
    inbox_count: usize,
}

fn my_player_summary(app: &SowApp, snapshot_tick: u64, my_pid: u16) -> Option<MyPlayerSummary> {
    LAST_MY_PLAYER.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((cached_tick, cached_pid, summary)) = *cache {
            if cached_tick == snapshot_tick && cached_pid == my_pid {
                return summary;
            }
        }
        let summary = app
            .sim
            .current_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.players.iter().find(|player| player.id == my_pid))
            .map(|player| MyPlayerSummary {
                gold: player.gold,
                troops: player.troops,
                max_troops: player.max_troops,
                alive: player.alive,
                has_spawned: player.has_spawned,
                team: player.team,
                leader: player.leader,
                kills: player.kills,
                deaths: player.deaths,
                assists: player.assists,
                inbox_count: player.alliance_requests.len() + player.resource_requests.len(),
            });
        *cache = Some((snapshot_tick, my_pid, summary));
        summary
    })
}

fn refresh_web_leaderboard_cache(app: &mut SowApp) {
    let Some(snapshot) = app.sim.current_snapshot.as_ref() else {
        LAST_WEB_RANKINGS_TICK.with(|tick| tick.set(None));
        app.ui.leaderboard_rankings.clear();
        return;
    };

    let snapshot_tick = snapshot.tick;
    let changed = LAST_WEB_RANKINGS_TICK.with(|tick| {
        if tick.get() == Some(snapshot_tick) {
            false
        } else {
            tick.set(Some(snapshot_tick));
            true
        }
    });
    if !changed {
        return;
    }

    let mut rankings: Vec<sow_ui::ui::hud::leaderboard::LeaderboardRanking> = snapshot
        .players
        .iter()
        .filter(|player| player.alive)
        .map(|player| sow_ui::ui::hud::leaderboard::LeaderboardRanking {
            id: player.id,
            tiles: player.tile_count,
            troops: player.troops,
            name: sow_core::player::display_name(player.id, &player.name, player.player_type),
            kills: player.kills,
            deaths: player.deaths,
            assists: player.assists,
        })
        .collect();
    rankings.sort_unstable_by(|a, b| {
        b.tiles
            .cmp(&a.tiles)
            .then_with(|| b.troops.total_cmp(&a.troops))
            .then_with(|| a.id.cmp(&b.id))
    });
    app.ui.leaderboard_rankings = rankings;
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
                WebMenuCommand::CancelPlacement => {
                    self.ui.app.hud_state.selected_building_kind = None;
                    self.ui.app.hud_state.selected_nuke_kind = None;
                }
                WebMenuCommand::ToggleInbox => {
                    self.ui.app.hud_state.show_alliance_inbox =
                        !self.ui.app.hud_state.show_alliance_inbox;
                }
                WebMenuCommand::AcceptAlliance { target_player_id } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::AcceptAlliance {
                        target_player: target_player_id,
                    });
                }
                WebMenuCommand::RejectAlliance { target_player_id } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::RejectAlliance {
                        target_player: target_player_id,
                    });
                }
                WebMenuCommand::OpenTransfer { target_player_id } => {
                    self.ui.app.hud_state.show_ask_panel = Some(target_player_id);
                    self.ui.app.hud_state.transfer_confirm_pending = false;
                }
                WebMenuCommand::CloseTransfer => {
                    self.ui.app.hud_state.show_ask_panel = None;
                    self.ui.app.hud_state.transfer_confirm_pending = false;
                }
                WebMenuCommand::SendResources {
                    target_player_id,
                    gold,
                    troops,
                } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::SendResources {
                        target_player: target_player_id,
                        gold: gold.max(0.0),
                        troops: troops.max(0.0),
                    });
                    self.ui.app.hud_state.show_ask_panel = None;
                    self.ui.app.hud_state.transfer_confirm_pending = false;
                }
                WebMenuCommand::RequestResources {
                    target_player_id,
                    gold,
                    troops,
                } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::RequestResources {
                        target_player: target_player_id,
                        gold: gold.max(0.0),
                        troops: troops.max(0.0),
                    });
                    self.ui.app.hud_state.show_ask_panel = None;
                    self.ui.app.hud_state.transfer_confirm_pending = false;
                }
                WebMenuCommand::AcceptResourceRequest { target_player_id } => {
                    self.send_intent(
                        sow_core::protocol::GameplayIntent::AcceptResourceRequest {
                            target_player: target_player_id,
                        },
                    );
                }
                WebMenuCommand::RejectResourceRequest { target_player_id } => {
                    self.send_intent(
                        sow_core::protocol::GameplayIntent::RejectResourceRequest {
                            target_player: target_player_id,
                        },
                    );
                }
                WebMenuCommand::ConfirmBetrayal => {
                    let warning = self
                        .ui
                        .app
                        .hud_state
                        .show_betrayal_warning
                        .clone()
                        .or_else(|| self.ui.app.hud_state.betrayal_warning_cached.clone());
                    if let Some((ally_id, intent)) = warning {
                        self.send_intent(sow_core::protocol::GameplayIntent::BreakAlliance {
                            target_player: ally_id,
                        });
                        self.send_intent(intent);
                    }
                    self.ui.app.hud_state.show_betrayal_warning = None;
                    self.ui.app.hud_state.betrayal_warning_cached = None;
                }
                WebMenuCommand::CancelBetrayal => {
                    self.ui.app.hud_state.show_betrayal_warning = None;
                    self.ui.app.hud_state.betrayal_warning_cached = None;
                }
                WebMenuCommand::CancelAttack { attack_id } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::CancelAttack { attack_id });
                }
                WebMenuCommand::RecallFleet { fleet_id } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::RecallFleet { fleet_id });
                }
                WebMenuCommand::Surrender => {
                    self.send_intent(sow_core::protocol::GameplayIntent::Resign);
                }
                WebMenuCommand::ToggleLeaderboard => {
                    self.ui.show_leaderboard = !self.ui.show_leaderboard;
                    if self.ui.show_leaderboard {
                        self.ui.tutorial_objectives_open = false;
                        #[cfg(any(feature = "dev", debug_assertions))]
                        {
                            self.ui.show_dev_sidebar = false;
                        }
                    }
                }
                WebMenuCommand::ReturnToMenu => {
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::LeaveLobby),
                    );
                }
                WebMenuCommand::ContinueObserving => {
                    self.ui.is_spectating = true;
                    self.ui.endgame_cache = None;
                }
                WebMenuCommand::ZoomIn => {
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::ZoomIn),
                    );
                }
                WebMenuCommand::ZoomOut => {
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::ZoomOut),
                    );
                }
                WebMenuCommand::CenterCamera => {
                    self.process_ui_actions(
                        &self.ui.egui_ctx.clone(),
                        Some(UiAction::CenterCamera),
                    );
                }
                WebMenuCommand::ToggleTutorialObjectives => {
                    if crate::hud::tutorial::tutorial_renders(
                        self.ui.tutorial_active,
                        self.net.is_offline,
                    ) {
                        self.ui.tutorial_objectives_open =
                            !self.ui.tutorial_objectives_open;
                        if self.ui.tutorial_objectives_open {
                            self.ui.show_leaderboard = false;
                            #[cfg(any(feature = "dev", debug_assertions))]
                            {
                                self.ui.show_dev_sidebar = false;
                            }
                        }
                    }
                }
                WebMenuCommand::ToggleDevSidebar => {
                    #[cfg(any(feature = "dev", debug_assertions))]
                    {
                        self.ui.show_dev_sidebar = !self.ui.show_dev_sidebar;
                        if self.ui.show_dev_sidebar {
                            self.ui.show_leaderboard = false;
                            self.ui.tutorial_objectives_open = false;
                        }
                    }
                }
                WebMenuCommand::SetDevConfig { field, value } => {
                    #[cfg(any(feature = "dev", debug_assertions))]
                    {
                        if self.ui.show_dev_sidebar && value.is_finite() {
                            sow_ui_kit::theme::dev_config::DevConfig::update(|config| {
                                match field {
                                    WebDevConfigField::Thickness => {
                                        config.thickness = value.clamp(0.0, 1.0)
                                    }
                                    WebDevConfigField::Darkness => {
                                        config.darkness = value.clamp(0.0, 1.0)
                                    }
                                    WebDevConfigField::ShoreThickness => {
                                        config.shore_thickness = value.clamp(0.0, 1.0)
                                    }
                                    WebDevConfigField::ConquestDuration => {
                                        config.conquest_duration = value.clamp(0.1, 10.0)
                                    }
                                    WebDevConfigField::TerritoryOpacity => {
                                        config.territory_opacity = value.clamp(0.0, 1.0)
                                    }
                                }
                            });
                        }
                    }
                    #[cfg(not(any(feature = "dev", debug_assertions)))]
                    let _ = (field, value);
                }
                WebMenuCommand::ResetDevConfig => {
                    #[cfg(any(feature = "dev", debug_assertions))]
                    {
                        let defaults = sow_ui_kit::theme::dev_config::DevConfig::default();
                        let mut config = sow_ui_kit::theme::dev_config::DevConfig::get();
                        config.thickness = defaults.thickness;
                        config.darkness = defaults.darkness;
                        config.shore_thickness = defaults.shore_thickness;
                        config.conquest_duration = defaults.conquest_duration;
                        config.territory_opacity = defaults.territory_opacity;
                        sow_ui_kit::theme::dev_config::DevConfig::set(config);
                    }
                }
                WebMenuCommand::ExpressEmoji { emoji, pinned } => {
                    self.send_intent(sow_core::protocol::GameplayIntent::ExpressEmoji {
                        emoji,
                        pinned,
                    });
                }
                WebMenuCommand::SetEmojiPinned { pinned } => {
                    self.ui.app.hud_state.pin_emoji = pinned;
                }
                WebMenuCommand::FocusPlayer { player_id } => {
                    if let Some(snap) = &self.sim.current_snapshot {
                        if let Some(player) = snap.players.iter().find(|p| p.id == player_id) {
                            if player.tile_count > 0 && player.alive {
                                let world_cx = player.centroid_x + 0.5;
                                let world_cy = player.centroid_y + 0.5;
                                self.input.camera_focus_target = Some((world_cx, world_cy));
                                self.input.target_zoom = 8.0;
                            }
                        }
                    }
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

fn hovered_tile_owner(app: &SowApp) -> (u32, u16) {
    if !app.input.camera_zoom.is_finite() || app.input.camera_zoom <= 0.0 {
        return (u32::MAX, 0);
    }
    let world_x = (app.input.last_mouse_x as f32 - app.input.camera_x) / app.input.camera_zoom;
    let world_y = (app.input.last_mouse_y as f32 - app.input.camera_y) / app.input.camera_zoom;
    let (col, row) = crate::render::world::movers::world_to_tile(world_x, world_y);
    if col < 0 || row < 0 || col >= app.sim.map_w as i32 || row >= app.sim.map_h as i32 {
        return (u32::MAX, 0);
    }
    let idx = (row * app.sim.map_w as i32 + col) as usize;
    let owner = app
        .gfx
        .map_renderer
        .as_ref()
        .and_then(|renderer| renderer.owners.get(idx).copied())
        .unwrap_or(0);
    (idx as u32, owner)
}

fn hud_publish_key(app: &SowApp) -> HudPublishKey {
    let hud = &app.ui.app.hud_state;
    let (hovered_tile, hovered_owner) = hovered_tile_owner(app);
    let tutorial_active = crate::hud::tutorial::tutorial_renders(
        app.ui.tutorial_active,
        app.net.is_offline,
    );
    let tutorial_objectives_open = tutorial_active && app.ui.tutorial_objectives_open;
    let (dev_sidebar_open, dev_config_key) = dev_config_key(app);
    let snapshot_tick = app
        .sim
        .current_snapshot
        .as_ref()
        .map(|snapshot| snapshot.tick)
        .unwrap_or(0);
    let cold_open = app.ui.show_leaderboard
        || hovered_owner != 0
        || hud.show_alliance_inbox
        || hud.show_ask_panel.is_some()
        || hud.show_betrayal_warning.is_some()
        || hud.sync_state.is_some()
        || tutorial_objectives_open;
    let my_pid = app.sim.my_player_id.unwrap_or(hud.my_player_id);
    let inbox_count = my_player_summary(app, snapshot_tick, my_pid)
        .map(|player| player.inbox_count)
        .unwrap_or(0);

    HudPublishKey {
        gold: hud.gold.to_bits(),
        troops: hud.troops.to_bits(),
        max_troops: hud.max_troops.to_bits(),
        troop_rate: hud.troop_rate.to_bits(),
        attack_ratio: hud.attack_ratio.to_bits(),
        spawn_timer_tenths: hud
            .spawn_timer_secs
            .map(|secs| (secs.max(0.0) * 10.0).round() as i32)
            .unwrap_or(-1),
        selected_building: hud
            .selected_building_kind
            .map(|kind| kind as u8)
            .unwrap_or(u8::MAX),
        selected_nuke: hud.selected_nuke_kind.map(|kind| kind as u8).unwrap_or(u8::MAX),
        building_costs: std::array::from_fn(|index| hud.building_costs[index].to_bits()),
        settings_mute: app.ui.app.settings_state.mute_all,
        settings_music_volume: app.ui.app.settings_state.music_volume.to_bits(),
        settings_reduced_motion: app.ui.app.settings_state.reduced_motion,
        leaderboard_open: app.ui.show_leaderboard,
        inbox_open: hud.show_alliance_inbox,
        transfer_target: hud.show_ask_panel,
        betrayal_open: hud.show_betrayal_warning.is_some(),
        sync_open: hud.sync_state.is_some(),
        inbox_count,
        notification_len: hud.hud_notifications.len(),
        is_spectating: app.ui.is_spectating,
        tutorial_active,
        tutorial_objectives_open,
        dev_sidebar_open,
        dev_thickness: dev_config_key[0],
        dev_darkness: dev_config_key[1],
        dev_shore_thickness: dev_config_key[2],
        dev_conquest_duration: dev_config_key[3],
        dev_territory_opacity: dev_config_key[4],
        snapshot_tick: if cold_open { snapshot_tick } else { 0 },
        hovered_tile,
        hovered_owner,
    }
}

#[cfg(any(feature = "dev", debug_assertions))]
fn dev_config_key(app: &SowApp) -> (bool, [u32; 5]) {
    if !app.ui.show_dev_sidebar {
        return (false, [0; 5]);
    }
    let config = sow_ui_kit::theme::dev_config::DevConfig::get();
    (
        true,
        [
            config.thickness.to_bits(),
            config.darkness.to_bits(),
            config.shore_thickness.to_bits(),
            config.conquest_duration.to_bits(),
            config.territory_opacity.to_bits(),
        ],
    )
}

#[cfg(not(any(feature = "dev", debug_assertions)))]
fn dev_config_key(_app: &SowApp) -> (bool, [u32; 5]) {
    (false, [0; 5])
}

#[cfg(any(feature = "dev", debug_assertions))]
fn dev_tools_payload(app: &SowApp) -> serde_json::Value {
    let open = app.ui.show_dev_sidebar;
    let mut payload = serde_json::json!({
        "available": true,
        "open": open,
    });
    if open {
        let config = sow_ui_kit::theme::dev_config::DevConfig::get();
        payload["config"] = serde_json::json!({
            "thickness": config.thickness,
            "darkness": config.darkness,
            "shore_thickness": config.shore_thickness,
            "conquest_duration": config.conquest_duration,
            "territory_opacity": config.territory_opacity,
        });
    }
    payload
}

#[cfg(not(any(feature = "dev", debug_assertions)))]
fn dev_tools_payload(_app: &SowApp) -> serde_json::Value {
    serde_json::json!({
        "available": false,
        "open": false,
    })
}

fn player_json(player: &sow_core::protocol::PlayerSnapshot, my_pid: u16, total_land_tiles: u32) -> serde_json::Value {
    let territory_pct = (player.tile_count as f32 / total_land_tiles.max(1) as f32).clamp(0.0, 1.0);
    serde_json::json!({
        "id": player.id,
        "name": &player.name,
        "troops": player.troops,
        "max_troops": player.max_troops,
        "tile_count": player.tile_count,
        "territory_pct": territory_pct,
        "is_alive": player.alive,
        "is_me": player.id == my_pid,
        "leader": leader_id(player.leader),
        "civilization": player.civilization.name(),
        "team": player.team,
        "active_emoji": &player.active_emoji,
        "disconnected": player.disconnected,
        "traitor": player.traitor,
        "kills": player.kills,
        "deaths": player.deaths,
        "assists": player.assists,
        "cap_x": player.centroid_x,
        "cap_y": player.centroid_y,
    })
}

fn build_leaderboard(snapshot: &sow_core::protocol::SimSnapshot, my_pid: u16) -> serde_json::Value {
    let mut players: Vec<&sow_core::protocol::PlayerSnapshot> = snapshot.players.iter().collect();
    players.sort_unstable_by(|a, b| {
        b.tile_count
            .cmp(&a.tile_count)
            .then_with(|| b.troops.total_cmp(&a.troops))
            .then_with(|| a.id.cmp(&b.id))
    });
    serde_json::Value::Array(
        players
            .into_iter()
            .map(|player| player_json(player, my_pid, snapshot.total_land_tiles))
            .collect(),
    )
}

fn build_hover_payload(
    snapshot: &sow_core::protocol::SimSnapshot,
    owner_id: u16,
    my_pid: u16,
) -> serde_json::Value {
    let Some(player) = snapshot.players.iter().find(|player| player.id == owner_id) else {
        return serde_json::Value::Null;
    };
    let mut cities = 0;
    let mut factories = 0;
    let mut ports = 0;
    let mut bunkers = 0;
    for building in &snapshot.buildings {
        if building.owner_id != owner_id {
            continue;
        }
        match building.kind {
            sow_core::game::BuildingKind::City => cities += 1,
            sow_core::game::BuildingKind::Factory => factories += 1,
            sow_core::game::BuildingKind::Port => ports += 1,
            sow_core::game::BuildingKind::Bunker => bunkers += 1,
        }
    }
    let mut payload = player_json(player, my_pid, snapshot.total_land_tiles);
    payload["cities"] = serde_json::json!(cities);
    payload["factories"] = serde_json::json!(factories);
    payload["ports"] = serde_json::json!(ports);
    payload["bunkers"] = serde_json::json!(bunkers);
    payload
}

fn cached_leaderboard_payload(
    snapshot: &sow_core::protocol::SimSnapshot,
    my_pid: u16,
) -> serde_json::Value {
    LAST_WEB_LEADERBOARD_PAYLOAD.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((tick, cached_pid, payload)) = cache.as_ref() {
            if *tick == snapshot.tick && *cached_pid == my_pid {
                return payload.clone();
            }
        }
        let payload = build_leaderboard(snapshot, my_pid);
        *cache = Some((snapshot.tick, my_pid, payload.clone()));
        payload
    })
}

fn cached_hover_payload(
    snapshot: &sow_core::protocol::SimSnapshot,
    hovered_tile: u32,
    owner_id: u16,
    my_pid: u16,
) -> serde_json::Value {
    if owner_id == 0 {
        return serde_json::Value::Null;
    }
    LAST_WEB_HOVER_PAYLOAD.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((tick, cached_tile, cached_owner, cached_pid, payload)) = cache.as_ref() {
            if *tick == snapshot.tick
                && *cached_tile == hovered_tile
                && *cached_owner == owner_id
                && *cached_pid == my_pid
            {
                return payload.clone();
            }
        }
        let payload = build_hover_payload(snapshot, owner_id, my_pid);
        *cache = Some((
            snapshot.tick,
            hovered_tile,
            owner_id,
            my_pid,
            payload.clone(),
        ));
        payload
    })
}

fn cached_inbox_payload(
    snapshot: &sow_core::protocol::SimSnapshot,
    my_pid: u16,
) -> serde_json::Value {
    LAST_WEB_INBOX_PAYLOAD.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((tick, cached_pid, payload)) = cache.as_ref() {
            if *tick == snapshot.tick && *cached_pid == my_pid {
                return payload.clone();
            }
        }
        let payload = build_inbox(snapshot, my_pid);
        *cache = Some((snapshot.tick, my_pid, payload.clone()));
        payload
    })
}

fn build_inbox(snapshot: &sow_core::protocol::SimSnapshot, my_pid: u16) -> serde_json::Value {
    let Some(me) = snapshot.players.iter().find(|player| player.id == my_pid) else {
        return serde_json::Value::Array(Vec::new());
    };
    let mut requests = Vec::new();
    for requester_id in &me.alliance_requests {
        if let Some(requester) = snapshot.players.iter().find(|player| player.id == *requester_id) {
            requests.push(serde_json::json!({
                "kind": "alliance",
                "requester_id": requester.id,
                "name": &requester.name,
                "leader": leader_id(requester.leader),
                "active": requester.alive,
            }));
        }
    }
    for request in &me.resource_requests {
        if let Some(requester) = snapshot.players.iter().find(|player| player.id == request.requester) {
            requests.push(serde_json::json!({
                "kind": "resources",
                "requester_id": requester.id,
                "name": &requester.name,
                "gold": request.gold,
                "troops": request.troops,
            }));
        }
    }
    serde_json::Value::Array(requests)
}

fn build_hud_payload(app: &SowApp) -> serde_json::Value {
    if app.ui.app.phase != sow_ui::ClientPhase::Playing {
        return serde_json::Value::Null;
    }

    let hud = &app.ui.app.hud_state;
    let my_pid = app.sim.my_player_id.unwrap_or(hud.my_player_id);
    let snapshot = app.sim.current_snapshot.as_ref();
    let snapshot_tick = snapshot.map(|snapshot| snapshot.tick).unwrap_or(0);
    let me = my_player_summary(app, snapshot_tick, my_pid);
    let match_over = !app.ui.is_spectating && snapshot.is_some_and(|snapshot| {
        snapshot.winner.is_some()
            || me.is_some_and(|player| !player.alive && player.has_spawned)
    });
    let is_winner = snapshot.is_some_and(|snapshot| {
        if let Some(team) = snapshot.winning_team {
            me.and_then(|player| player.team) == Some(team)
        } else {
            snapshot.winner == Some(my_pid)
        }
    });
    let winner_name = snapshot
        .and_then(|snapshot| {
            snapshot
                .winner
                .and_then(|winner_id| snapshot.players.iter().find(|player| player.id == winner_id))
        })
        .map(|player| player.name.clone())
        .unwrap_or_default();
    let (hovered_tile, hovered_owner) = hovered_tile_owner(app);
    let selected_building = hud.selected_building_kind.map(|kind| match kind {
        sow_core::game::BuildingKind::City => "City",
        sow_core::game::BuildingKind::Factory => "Factory",
        sow_core::game::BuildingKind::Port => "Port",
        sow_core::game::BuildingKind::Bunker => "Bunker",
    });
    let costs = &hud.building_costs;
    // The store is only shown after a match. Keep its catalog out of the hot HUD path,
    // and use the browser clock because SystemTime::now() panics on wasm32.
    let featured_skin = if match_over {
        let rotation_period = ((js_sys::Date::now().max(0.0) / 1000.0) as u64)
            / sow_data::commerce::ROTATION_PERIOD_SECS;
        sow_data::commerce::catalog_for_profile(
            &app.progress.owned_leaders,
            &app.progress.owned_skins,
            app.progress.laurels,
            app.progress.gems,
            rotation_period,
        )
        .skins
        .into_iter()
        .find(|skin| !skin.owned)
    } else {
        None
    };
    let mut payload = serde_json::json!({
        "gold": me.map(|player| player.gold).unwrap_or(hud.gold),
        "troops": me.map(|player| player.troops).unwrap_or(hud.troops),
        "max_troops": me.map(|player| player.max_troops).unwrap_or(hud.max_troops),
        "troop_rate": hud.troop_rate,
        "attack_ratio": hud.attack_ratio,
        "spawn_timer_secs": hud.spawn_timer_secs,
        "selected_building": selected_building,
        "selected_nuke": hud.selected_nuke_kind.is_some(),
        "pin_emoji": hud.pin_emoji,
        "building_costs": {
            "city": costs[0],
            "bunker": costs[1],
            "factory": costs[2],
            "port": costs[3],
        },
        "fps": app.time.current_fps,
        "ping": app.net.last_ping_time.elapsed().as_millis() as u32,
        "hovered_tile": if hovered_tile == u32::MAX { serde_json::Value::Null } else { serde_json::json!(hovered_tile) },
        "hovered": serde_json::Value::Null,
        "inbox_count": me.map(|player| player.inbox_count).unwrap_or(0),
        "match_over": match_over,
        "is_spectating": app.ui.is_spectating,
        "quests": {
            "available": crate::hud::tutorial::tutorial_renders(
                app.ui.tutorial_active,
                app.net.is_offline,
            ),
            "open": crate::hud::tutorial::tutorial_renders(
                app.ui.tutorial_active,
                app.net.is_offline,
            ) && app.ui.tutorial_objectives_open,
        },
        "dev_tools": dev_tools_payload(app),
        "is_winner": is_winner,
        "winner_name": winner_name,
        "featured_skin": featured_skin.map(|skin| serde_json::json!({
            "id": skin.id,
            "name": skin.name,
            "asset_path": skin.asset_path,
            "cost_gems": skin.cost_gems,
        })).unwrap_or(serde_json::Value::Null),
        "player_leader": me.map(|player| leader_id(player.leader)),
        "player_kda": {
            "kills": me.map(|player| player.kills).unwrap_or(0),
            "deaths": me.map(|player| player.deaths).unwrap_or(0),
            "assists": me.map(|player| player.assists).unwrap_or(0),
        },
    });

    if let Some(snapshot) = snapshot {
        if hovered_owner != 0 {
            payload["hovered"] = cached_hover_payload(snapshot, hovered_tile, hovered_owner, my_pid);
        }
        if app.ui.show_leaderboard {
            payload["leaderboard"] = cached_leaderboard_payload(snapshot, my_pid);
        }
        if hud.show_alliance_inbox {
            payload["inbox"] = cached_inbox_payload(snapshot, my_pid);
        }
        if let Some(target_id) = hud.show_ask_panel {
            if let Some(target) = snapshot.players.iter().find(|player| player.id == target_id) {
                payload["transfer"] = serde_json::json!({
                    "target_id": target.id,
                    "target_name": &target.name,
                    "target_alive": target.alive,
                    "confirm_pending": hud.transfer_confirm_pending,
                });
            }
        }
    }

    if hud.show_betrayal_warning.is_some() || hud.betrayal_warning_cached.is_some() {
        let warning = hud
            .show_betrayal_warning
            .as_ref()
            .or(hud.betrayal_warning_cached.as_ref());
        if let Some((ally_id, _)) = warning {
            payload["betrayal"] = serde_json::json!({
                "ally_id": ally_id,
                "ally_name": snapshot
                    .and_then(|snapshot| snapshot.players.iter().find(|player| player.id == *ally_id))
                    .map(|player| player.name.clone())
                    .unwrap_or_else(|| "Ally".to_string()),
            });
        }
    }
    if let Some(sync) = &hud.sync_state {
        payload["sync"] = serde_json::to_value(sync).unwrap_or(serde_json::Value::Null);
    }
    payload["notifications"] = serde_json::Value::Array(
        hud.hud_notifications
            .iter()
            .map(|notice| serde_json::json!({ "message": &notice.message }))
            .collect(),
    );
    if match_over {
        let reward = app.ui.reward_cache.or_else(|| {
            me.map(|player| sow_data::rewards::calculate(sow_data::rewards::RewardInput {
                won: is_winner,
                players_defeated: app.progress_session_defeats.players,
                empires_defeated: app.progress_session_defeats.empires,
                tribes_defeated: app.progress_session_defeats.tribes,
                kills: player.kills,
                assists: player.assists,
                tutorial: app.sim.config.tutorial,
            }))
        });
        if let Some(reward) = reward {
            payload["rewards"] = serde_json::json!({
                "xp": reward.xp,
                "leader_xp": reward.leader_xp,
                "laurels": reward.laurels,
            });
        }
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
pub(crate) fn publish_state(app: &mut SowApp) {
    refresh_web_leaderboard_cache(app);
    if !publish_due() {
        return;
    }

    let state = &app.ui.app.main_menu_state;
    let progress = &app.progress;

    let payload = if app.ui.app.phase == sow_ui_kit::ClientPhase::Playing {
        let hud_key = hud_publish_key(app);
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
        let hud_payload = build_hud_payload(app);
        serde_json::json!({
            "phase": "Playing",
            "hud": hud_payload,
            "settings": {
                "mute_all": app.ui.app.settings_state.mute_all,
                "music_volume": app.ui.app.settings_state.music_volume,
                "reduced_motion": app.ui.app.settings_state.reduced_motion,
            },
        })
    } else {
        LAST_HUD_KEY.with(|last| *last.borrow_mut() = None);
        LAST_MY_PLAYER.with(|cache| *cache.borrow_mut() = None);
        LAST_WEB_RANKINGS_TICK.with(|tick| tick.set(None));
        LAST_WEB_LEADERBOARD_PAYLOAD.with(|cache| *cache.borrow_mut() = None);
        LAST_WEB_HOVER_PAYLOAD.with(|cache| *cache.borrow_mut() = None);
        LAST_WEB_INBOX_PAYLOAD.with(|cache| *cache.borrow_mut() = None);
        app.ui.leaderboard_rankings.clear();
        let rotation_period = ((js_sys::Date::now().max(0.0) / 1000.0) as u64)
            / sow_data::commerce::ROTATION_PERIOD_SECS;
        let store_catalog = sow_data::commerce::catalog_for_profile(
            &progress.owned_leaders,
            &progress.owned_skins,
            progress.laurels,
            progress.gems,
            rotation_period,
        );
        let leaders: Vec<serde_json::Value> = sow_core::player::Leader::ALL
            .into_iter()
            .map(|leader| {
                let slug = sow_data::commerce::leader_id(leader);
                let free_rotation = store_catalog.free_leaders.iter().any(|id| id == slug);
                let owned = progress.owned_leaders.contains(slug);
                serde_json::json!({
                    "id": leader_id(leader),
                    "name": leader.name(),
                    "civilization": leader.civilization().name(),
                    "perk": leader.perk_description(),
                    "slug": slug,
                    "free_rotation": free_rotation,
                    "owned": owned,
                    "available": free_rotation || owned,
                    "cost_laurels": sow_data::commerce::LEADER_UNLOCK_COST_LAURELS,
                    "cost_gems": sow_data::commerce::LEADER_UNLOCK_COST_GEMS,
                })
            })
            .collect();
        let map_catalog: Vec<serde_json::Value> = app
            .ui
            .app
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
            "phase": phase_name(app.ui.app.phase),
            "loader_job": splash_job_name(&app.ui.app.splash_state.job),
            "loader_progress": app.ui.app.splash_state.progress.clamp(0.0, 1.0),
            "loader_status": app
                .ui
                .app
                .splash_state
                .status_override
                .as_deref()
                .unwrap_or(app.ui.app.splash_state.status_text.as_str()),
            "loader_done": app.ui.app.splash_state.done,
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
            "gems": progress.gems,
            "selected_skin": progress.selected_skin,
            "store": store_catalog,
            "native_purchase_scheme": "sow://purchase",
            "native_restore_scheme": "sow://restore",
            "purchase_user_id": app.profile_public_id,
            "public_profile_id": app.profile_public_id,
            "profile_stats": {
                "wins": progress.wins,
                "matches_played": progress.matches_played,
                "players_defeated": progress.players_defeated,
                "empires_defeated": progress.empires_defeated,
                "tribes_defeated": progress.tribes_defeated,
                "kills": progress.kills,
                "deaths": progress.deaths,
                "assists": progress.assists,
                "leader_xp": progress.leader_xp,
            },
            "leaders": leaders,
            "map_catalog": map_catalog,
            "custom_game_config": &*state.custom_game_config,
            "custom_game_is_private": state.custom_game_is_private,
            "custom_game_is_sp": state.custom_game_is_sp,
            "hud": serde_json::Value::Null,
            "settings": {
                "mute_all": app.ui.app.settings_state.mute_all,
                "music_volume": app.ui.app.settings_state.music_volume,
                "reduced_motion": app.ui.app.settings_state.reduced_motion,
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
    let js_str = JsValue::from_str(&serialized);
    let _ = js_sys::Reflect::set(
        window.as_ref(),
        &JsValue::from_str("SOW_MENU_STATE"),
        &js_str,
    );
    if let Ok(func_val) = js_sys::Reflect::get(window.as_ref(), &JsValue::from_str("SOW_onStateUpdate")) {
        if let Ok(func) = func_val.dyn_into::<js_sys::Function>() {
            let _ = func.call1(window.as_ref(), &js_str);
        }
    }
}
