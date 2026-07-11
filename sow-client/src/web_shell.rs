use crate::app::SowApp;
use sow_ui_kit::ClientPhase;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerSetup {
    pub name: String,
    pub clan_tag: String,
    pub civilization: sow_core::player::Civilization,
    pub leader: sow_core::player::Leader,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameConfigPatch {
    pub max_players: Option<u32>,
    pub bot_count: Option<u32>,
    pub nation_count: Option<u32>,
    pub bot_difficulty: Option<sow_core::game_config::BotDifficulty>,
    pub seed: Option<u64>,
    pub map_name: Option<String>,
    pub game_mode: Option<String>,
    pub random_spawn: Option<bool>,
    pub map_control_win_percentage: Option<f32>,
    pub tick_rate_ms: Option<f32>,
    pub global_speed_multiplier: Option<f64>,
    pub attack_cost_enemy: Option<f64>,
    pub attack_cost_neutral: Option<f64>,
    pub terrain_multiplier_highland: Option<f64>,
    pub terrain_multiplier_mountain: Option<f64>,
    pub max_tiles_per_tick: Option<f64>,
    pub momentum_divisor: Option<f64>,
    pub starting_troops: Option<f64>,
    pub starting_gold: Option<f64>,
    pub gold_base_income: Option<f64>,
    pub buildings_enabled: Option<bool>,
    pub tutorial: Option<bool>,
}

impl GameConfigPatch {
    pub fn apply_patch(&self, config: &mut sow_core::game_config::GameConfig, asset_loader: &sow_ui::ui::asset_loader::AssetLoader) {
        if let Some(val) = self.max_players { config.max_players = val; }
        if let Some(val) = self.bot_count { config.bot_count = val; }
        if let Some(val) = self.nation_count { config.nation_count = val; }
        if let Some(val) = self.bot_difficulty { config.bot_difficulty = val; }
        if let Some(val) = self.seed { config.seed = val; }
        if let Some(ref val) = self.map_name {
            config.map_name = val.clone();
            if let Some(catalog) = &asset_loader.map_catalog {
                if let Some(entry) = sow_core::maps::catalog_lookup(catalog, val) {
                    config.map_width = entry.width;
                    config.map_height = entry.height;
                }
            }
        }
        if let Some(ref val) = self.game_mode { config.game_mode = val.clone(); }
        if let Some(val) = self.random_spawn { config.random_spawn = val; }
        if let Some(val) = self.map_control_win_percentage { config.map_control_win_percentage = val; }
        if let Some(val) = self.tick_rate_ms { config.tick_rate_ms = val; }
        if let Some(val) = self.global_speed_multiplier { config.global_speed_multiplier = val; }
        if let Some(val) = self.attack_cost_enemy { config.attack_cost_enemy = val; }
        if let Some(val) = self.attack_cost_neutral { config.attack_cost_neutral = val; }
        if let Some(val) = self.terrain_multiplier_highland { config.terrain_multiplier_highland = val; }
        if let Some(val) = self.terrain_multiplier_mountain { config.terrain_multiplier_mountain = val; }
        if let Some(val) = self.max_tiles_per_tick { config.max_tiles_per_tick = val; }
        if let Some(val) = self.momentum_divisor { config.momentum_divisor = val; }
        if let Some(val) = self.starting_troops { config.starting_troops = val; }
        if let Some(val) = self.starting_gold { config.starting_gold = val; }
        if let Some(val) = self.gold_base_income { config.gold_base_income = val; }
        if let Some(val) = self.buildings_enabled { config.buildings_enabled = val; }
        if let Some(val) = self.tutorial { config.tutorial = val; }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BootAction {
    #[serde(rename = "type")]
    pub action_type: String, // "quick_play", "join_lobby", "host_custom", "offline", "tutorial"
    pub player: PlayerSetup,
    pub lobby_id: Option<u64>,
    pub password: Option<String>,
    pub visibility: Option<String>, // "public", "private"
    pub config: Option<GameConfigPatch>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SettingsPatch {
    pub music_volume: Option<f32>,
    pub sfx_volume: Option<f32>,
    pub mute_all: Option<bool>,
    pub language: Option<sow_i18n::Language>,
    pub reduced_motion: Option<bool>,
    pub show_fps_ping: Option<bool>,
    pub custom_theme: Option<bool>,
}

impl SettingsPatch {
    pub fn apply(&self, state: &mut sow_ui::ui::settings::SettingsState) {
        if let Some(val) = self.music_volume { state.music_volume = val; }
        if let Some(val) = self.sfx_volume { state.sfx_volume = val; }
        if let Some(val) = self.mute_all { state.mute_all = val; }
        if let Some(val) = self.language { state.language = val; }
        if let Some(val) = self.reduced_motion { state.reduced_motion = val; }
        if let Some(val) = self.show_fps_ping { state.show_fps_ping = val; }
        if let Some(val) = self.custom_theme { state.custom_theme = val; }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShellCommand {
    #[serde(rename = "type")]
    pub cmd_type: String, // "settings", "lobby", "leave_lobby", "exit_match", "focus"
    pub patch: Option<SettingsPatch>,
    pub cmd: Option<String>, // "force_start", "kick", "ban", "set_team", "ready"
    pub target_player_id: Option<u16>,
    pub focused: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LobbyPlayerState {
    pub player_id: u16,
    pub name: String,
    pub leader: sow_core::player::Leader,
    pub is_ready: bool,
    pub download_progress: u8,
    pub team: Option<sow_core::protocol::Team>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PublishLobbyState {
    pub lobby_id: u64,
    pub is_private: bool,
    pub map_name: String,
    pub game_mode: String,
    pub max_players: u32,
    pub time_remaining: f32,
    pub is_starting: bool,
    pub am_host: bool,
    pub my_player_id: Option<u16>,
    pub players: Vec<LobbyPlayerState>,
}

pub fn web_shell_poll(app: &mut SowApp) {
    if !crate::store_portals::web_shell_mode() {
        return;
    }

    if app.ui.app.phase == ClientPhase::MainMenu && !app.ui.app.main_menu_state.is_waiting {
        if let Some(action_json) = crate::store_portals::take_boot_action() {
            if let Ok(action) = serde_json::from_str::<BootAction>(&action_json) {
                // Seed player settings
                app.ui.app.main_menu_state.player_name = action.player.name;
                app.ui.app.main_menu_state.clan_tag = action.player.clan_tag;
                app.ui.app.main_menu_state.selected_civilization = action.player.civilization;
                app.ui.app.main_menu_state.selected_leader = action.player.leader;

                match action.action_type.as_str() {
                    "quick_play" => {
                        app.request_join(None, false, None, None);
                    }
                    "join_lobby" => {
                        if let Some(lobby_id) = action.lobby_id {
                            app.request_join(Some(lobby_id), false, None, action.password);
                        }
                    }
                    "host_custom" => {
                        let is_private = action.visibility.as_deref() == Some("private");
                        let mut config = sow_core::game_config::GameConfig::default();
                        if let Some(patch) = action.config {
                            patch.apply_patch(&mut config, &app.ui.app.asset_loader);
                        }
                        app.request_join(None, is_private, Some(Box::new(config)), action.password);
                    }
                    "offline" | "tutorial" => {
                        let is_tutorial = action.action_type == "tutorial";
                        let mut config = sow_core::game_config::GameConfig::default();
                        if let Some(patch) = action.config {
                            patch.apply_patch(&mut config, &app.ui.app.asset_loader);
                        }
                        app.start_offline_match(config, is_tutorial);
                    }
                    _ => {}
                }
            }
        }
    }

    web_shell_drain_commands(app);
}

pub fn web_shell_drain_commands(app: &mut SowApp) {
    let commands = crate::store_portals::drain_ui_commands();
    for cmd_json in commands {
        if let Ok(cmd) = serde_json::from_str::<ShellCommand>(&cmd_json) {
            match cmd.cmd_type.as_str() {
                "settings" => {
                    if let Some(patch) = cmd.patch {
                        patch.apply(&mut app.ui.app.settings_state);
                    }
                }
                "lobby" => {
                    let lobby_id = app.ui.app.main_menu_state.joined_lobby_id.or(app.sim.my_lobby_id);
                    let my_player_id = app.sim.my_player_id.or(app.ui.app.main_menu_state.my_player_id);

                    if let (Some(l_id), Some(p_id)) = (lobby_id, my_player_id) {
                        let client_msg = match cmd.cmd.as_deref() {
                            Some("force_start") => Some(sow_core::protocol::ClientMessage::ForceStart {
                                lobby_id: l_id,
                                player_id: p_id,
                            }),
                            Some("kick") => cmd.target_player_id.map(|t| sow_core::protocol::ClientMessage::Kick {
                                lobby_id: l_id,
                                target_player_id: t,
                            }),
                            Some("ban") => cmd.target_player_id.map(|t| sow_core::protocol::ClientMessage::Ban {
                                lobby_id: l_id,
                                target_player_id: t,
                            }),
                            Some("set_team") => cmd.target_player_id.map(|t| sow_core::protocol::ClientMessage::SetPlayerTeam {
                                lobby_id: l_id,
                                target_player_id: t,
                            }),
                            Some("ready") => Some(sow_core::protocol::ClientMessage::Ready {
                                lobby_id: l_id,
                                player_id: p_id,
                            }),
                            _ => None,
                        };

                        if let Some(msg) = client_msg {
                            if let Ok(json) = bincode::serialize(&msg) {
                                if let Some(c) = app.net.client.as_ref() {
                                    c.send(json);
                                }
                            }
                        }
                    }
                }
                "leave_lobby" => {
                    crate::store_portals::left_room();
                    if let Some(c) = app.net.client.as_ref() {
                        let leave = sow_core::protocol::ClientMessage::Leave {};
                        if let Ok(json) = bincode::serialize(&leave) {
                            c.send(json);
                        }
                    }
                    app.ui.app.main_menu_state.in_private_match = false;
                    app.ui.app.main_menu_state.is_lobby_host = false;
                    app.ui.app.main_menu_state.my_player_id = None;
                    app.input.camera_x = 0.0;
                    app.input.camera_y = 0.0;
                    app.input.camera_zoom = 2.0;
                    app.input.target_zoom = 2.0;
                    app.net.client = None;
                    app.begin_exit_to_main_menu(false);
                }
                "exit_match" => {
                    app.begin_exit_to_main_menu(true);
                }
                "focus" => {
                    if let Some(focused) = cmd.focused {
                        app.input.input_focused = focused;
                    }
                }
                _ => {}
            }
        }
    }
}

pub fn publish_lobby_state(app: &SowApp) {
    if !crate::store_portals::web_shell_mode() {
        return;
    }

    let lobby_id = match app.ui.app.main_menu_state.joined_lobby_id.or(app.sim.my_lobby_id) {
        Some(id) => id,
        None => return,
    };

    if let Some(lobby) = app.ui.app.main_menu_state.lobbies.iter().find(|l| l.id == lobby_id) {
        let is_private = app.ui.app.main_menu_state.in_private_match;
        let map_name = lobby.map_name.clone();
        let game_mode = lobby.game_mode.clone();
        let max_players = lobby.max_players;
        let time_remaining = lobby.timer_secs;
        let is_starting = lobby.is_counting_down;

        let am_host = app.ui.app.main_menu_state.is_lobby_host;
        let my_player_id = app.sim.my_player_id.or(app.ui.app.main_menu_state.my_player_id);

        let players: Vec<LobbyPlayerState> = lobby.players.iter().map(|p| {
            LobbyPlayerState {
                player_id: p.player_id,
                name: p.name.clone(),
                leader: p.leader,
                is_ready: p.is_ready,
                download_progress: p.download_progress,
                team: p.team,
            }
        }).collect();

        let state = PublishLobbyState {
            lobby_id,
            is_private,
            map_name,
            game_mode,
            max_players,
            time_remaining,
            is_starting,
            am_host,
            my_player_id,
            players,
        };

        if let Ok(json) = serde_json::to_string(&state) {
            crate::store_portals::emit_lobby_state(&json);
        }
    }
}
