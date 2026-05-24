use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum Language {
    English,
    Spanish,
    French,
    German,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MainMenuStrings {
    pub play_tutorial: String,
    pub single_player: String,
    pub ranked_match: String,
    pub settings: String,
    pub map_editor: String,
    pub waiting_for_lobby: String,
    pub nickname_hint: String,
    pub connecting: String,
    pub connection_error_title: String,
    pub connection_error_header: String,
    pub dismiss: String,
    pub single_player_skirmish: String,
    pub config_simulation: String,
    pub map_selection: String,
    pub bot_difficulty: String,
    pub tribes_count: String,
    pub nations_count: String,
    pub random_spawning: String,
    pub simulation_speed: String,
    pub no_preview: String,
    pub back: String,
    pub start_simulation: String,
    pub no_maps_found: String,
    pub loading_maps: String,
    pub matchmaking_established: String,
    pub awaiting_combat_criteria: String,
    pub establishing_tactical_comm: String,
    pub leave_lobby: String,
    pub tactical_briefing: String,
    pub holographic_scanning: String,
    pub free_for_all: String,
    pub team_tactics: String,
    pub simulation: String,
    pub channel: String,
    pub slots: String,
    pub lobby_channel_label: String,
    pub max_sector_slots: String,
    pub deployment_engine: String,
    pub deployment_engine_val: String,
    pub ready_room: String,
    pub ready: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SettingsStrings {
    pub title: String,
    pub graphics_quality: String,
    pub audio: String,
    pub language: String,
    pub quality_low: String,
    pub quality_medium: String,
    pub quality_high: String,
    pub mute_all: String,
    pub music_volume: String,
    pub sfx_volume: String,
    pub back_button: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoadingScreenStrings {
    pub loading: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EndgameStrings {
    pub victory_title: String,
    pub defeat_title: String,
    pub victory_subtitle: String,
    pub defeat_subtitle: String,
    pub return_to_lobby: String,
    pub spectate: String,
    pub victory_flavors: Vec<String>,
    pub defeat_flavors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HudStrings {
    pub hover_settings: String,
    pub hover_exit: String,
    pub hover_zoom_in: String,
    pub hover_zoom_out: String,
    pub hover_center_camera: String,
    pub retreating_label: String,
    pub hover_retaliate: String,
    pub default_player_name: String,
    pub wilderness_player_name: String,
    pub naval_invasion_label: String,
    pub spawn_choose_location: String,
    pub spawn_seconds_remaining: String,
    pub overlay_waiting_players: String,
    pub overlay_all_ready: String,
    pub overlay_stabilizing: String,
    pub overlay_starting_in: String,
    pub overlay_seconds_short: String,
    pub overlay_players_ready: String,
    pub status_own: String,
    pub status_ally: String,
    pub status_enemy: String,
    pub status_neutral: String,
    pub status_tile_prefix: String,
    pub btn_info: String,
    pub btn_delete: String,
    pub btn_fleft: String,
    pub btn_ally: String,
    pub btn_build: String,
    pub btn_attack: String,
    pub inbox_title: String,
    pub inbox_empty: String,
    pub inbox_wants_ally: String,
    pub btn_accept: String,
    pub btn_reject: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TutorialStrings {
    pub commander_title: String,
    pub step_welcome_title: String,
    pub step_welcome_desc: String,
    pub step_expansion_title: String,
    pub step_expansion_desc: String,
    pub step_combat_title: String,
    pub step_combat_desc: String,
    pub step_complete_title: String,
    pub step_complete_desc: String,
    pub btn_next: String,
    pub btn_finish: String,
    pub btn_skip: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MapEditorStrings {
    pub title: String,
    pub btn_new: String,
    pub btn_export: String,
    pub btn_exit: String,
    pub label_size: String,
    pub heading_brush: String,
    pub label_terrain: String,
    pub paint_plains: String,
    pub paint_highlands: String,
    pub paint_mountains: String,
    pub paint_lake: String,
    pub paint_ocean: String,
    pub paint_shoreline: String,
    pub label_brush_size: String,
    pub label_strength: String,
    pub heading_instructions: String,
    pub instructions_body: String,
    pub heading_spawns: String,
    pub btn_place_spawn: String,
    pub label_placed_spawns: String,
    pub hover_nation_name: String,
    pub hover_flag: String,
    pub label_metadata_name: String,
    pub win_new_title: String,
    pub label_width: String,
    pub label_height: String,
    pub btn_create_map: String,
    pub msg_blank_created: String,
    pub msg_spawn_placed: String,
    pub msg_spawn_removed: String,
    pub msg_compiling: String,
    pub msg_saved: String,
    pub msg_write_failed: String,
}

#[derive(Debug, Clone)]
pub struct LanguageStrings {
    pub main_menu: MainMenuStrings,
    pub settings: SettingsStrings,
    pub loading_screen: LoadingScreenStrings,
    pub endgame: EndgameStrings,
    pub hud: HudStrings,
    pub tutorial: TutorialStrings,
    pub map_editor: MapEditorStrings,
}

static EN_STRINGS: OnceLock<LanguageStrings> = OnceLock::new();
static ES_STRINGS: OnceLock<LanguageStrings> = OnceLock::new();

pub fn get(lang: Language) -> &'static LanguageStrings {
    match lang {
        Language::Spanish => ES_STRINGS.get_or_init(|| {
            load_language(
                include_str!("../strings/es/main_menu.toml"),
                include_str!("../strings/es/settings.toml"),
                include_str!("../strings/es/loading_screen.toml"),
                include_str!("../strings/es/endgame.toml"),
                include_str!("../strings/es/hud.toml"),
                include_str!("../strings/es/tutorial.toml"),
                include_str!("../strings/es/map_editor.toml"),
            )
        }),
        _ => EN_STRINGS.get_or_init(|| {
            load_language(
                include_str!("../strings/en/main_menu.toml"),
                include_str!("../strings/en/settings.toml"),
                include_str!("../strings/en/loading_screen.toml"),
                include_str!("../strings/en/endgame.toml"),
                include_str!("../strings/en/hud.toml"),
                include_str!("../strings/en/tutorial.toml"),
                include_str!("../strings/en/map_editor.toml"),
            )
        }),
    }
}

fn load_language(
    main_menu_toml: &str,
    settings_toml: &str,
    loading_screen_toml: &str,
    endgame_toml: &str,
    hud_toml: &str,
    tutorial_toml: &str,
    map_editor_toml: &str,
) -> LanguageStrings {
    LanguageStrings {
        main_menu: toml::from_str(main_menu_toml).expect("Failed to parse main_menu.toml"),
        settings: toml::from_str(settings_toml).expect("Failed to parse settings.toml"),
        loading_screen: toml::from_str(loading_screen_toml)
            .expect("Failed to parse loading_screen.toml"),
        endgame: toml::from_str(endgame_toml).expect("Failed to parse endgame.toml"),
        hud: toml::from_str(hud_toml).expect("Failed to parse hud.toml"),
        tutorial: toml::from_str(tutorial_toml).expect("Failed to parse tutorial.toml"),
        map_editor: toml::from_str(map_editor_toml).expect("Failed to parse map_editor.toml"),
    }
}
