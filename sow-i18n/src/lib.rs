use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    English,
    Spanish,
    French,
    German,
}

impl Language {
    pub fn from_locale(locale: &str) -> Self {
        let normalized = locale.to_lowercase();
        if normalized.starts_with("es") {
            Language::Spanish
        } else if normalized.starts_with("fr") {
            Language::French
        } else if normalized.starts_with("de") {
            Language::German
        } else {
            Language::English
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MainMenuStrings {
    pub host_private_game: String,
    pub ranked_match: String,
    pub settings: String,
    pub waiting_for_lobby: String,
    pub no_lobbies_yet: String,
    pub nickname_hint: String,
    pub connecting: String,
    pub connection_error_title: String,
    pub connection_error_header: String,
    pub connection_error_hint: String,
    pub dismiss: String,
    pub connection_retry: String,
    pub config_simulation: String,
    pub map_selection: String,
    pub bot_difficulty: String,
    pub tribes_count: String,
    pub nations_count: String,
    pub random_spawning: String,
    pub simulation_speed: String,
    pub no_preview: String,
    pub loading_thumbnail: String,
    pub loading_leader_portrait: String,
    pub thumbnail_load_failed: String,
    pub back: String,
    pub start_simulation: String,
    pub no_maps_found: String,
    pub by_playing_you_agree: String,
    pub and_the: String,
    pub loading_maps: String,
    pub downloading_map: String,
    pub establishing_tactical_comm: String,
    pub leave_lobby: String,
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
    pub ready: String,
    pub credits_link: String,
    pub start_game: String,
    pub lobby_code_label: String,
    // Custom Game screen (unified offline skirmish + online lobby creation)
    pub create_lobby_btn: String,
    pub custom_game_title: String,
    pub custom_offline_toggle: String,
    pub custom_online_toggle: String,
    pub custom_offline_hint: String,
    pub custom_online_hint: String,
    pub visibility_label: String,
    pub visibility_public: String,
    pub visibility_private: String,
    pub password_label: String,
    pub password_hint: String,
    pub max_players_label: String,
    pub game_mode_label: String,
    pub seed_label: String,
    // Home screen: inline game browser + create
    pub create_game_btn: String,
    pub quick_match_label: String,
    pub game_browser_title: String,
    pub filter_all: String,
    pub lobby_code_hint: String,
    pub join_private_btn: String,
    pub no_public_games: String,
    pub wrong_password: String,
    pub link_conflict_title: String,
    pub link_conflict_body: String,
    pub link_conflict_guest_label: String,
    pub link_conflict_platform_label: String,
    pub link_conflict_level: String,
    pub link_conflict_keep_guest: String,
    pub link_conflict_keep_platform: String,
    pub sign_in: String,
    // Host moderation + lobby removal notices
    pub kick_btn: String,
    pub ban_btn: String,
    pub move_team_btn: String,
    pub host_label: String,
    pub players_label: String,
    pub bots_label: String,
    pub notice_host_left_title: String,
    pub notice_host_left_body: String,
    pub notice_kicked_title: String,
    pub notice_kicked_body: String,
    pub notice_banned_title: String,
    pub notice_banned_body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsStrings {
    pub title: String,
    pub graphics_quality: String,
    pub audio: String,
    pub language: String,
    pub quality_low: String,
    pub quality_medium: String,
    pub quality_high: String,
    pub quality_low_help: String,
    pub quality_medium_help: String,
    pub quality_high_help: String,
    pub mute_all: String,
    pub music_volume: String,
    pub sfx_volume: String,
    pub lang_english: String,
    pub lang_spanish: String,
    pub back_button: String,
    pub credits_licenses: String,
    pub privacy_policy: String,
    pub terms_of_service: String,
    pub settings_applied: String,
    pub reduced_motion: String,
    pub reduced_motion_help: String,
    pub show_fps_ping: String,
    pub show_fps_ping_help: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadingScreenStrings {
    pub loading: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndgameStrings {
    pub victory_title: String,
    pub defeat_title: String,
    pub victory_subtitle: String,
    pub defeat_subtitle: String,
    pub return_to_lobby: String,
    pub play_again: String,
    pub spectate: String,
    pub winner_emerged: String,
    #[serde(default = "default_team_victory_subtitle")]
    pub team_victory_subtitle: String,
    #[serde(default = "default_team_defeat_subtitle")]
    pub team_defeat_subtitle: String,
    #[serde(default = "default_team_winner_emerged")]
    pub team_winner_emerged: String,
    pub victory_flavors: Vec<String>,
    pub defeat_flavors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub event_log_title: String,
    pub event_log_empty: String,
    pub event_log_clear: String,
    pub event_time_seconds: String,
    pub event_time_minutes: String,
    pub battle_log_empty: String,
    pub naval_fleet_label: String,
    pub emoji_panel_title: String,
    pub hover_express_emoji: String,
    pub hover_battle_log: String,
    pub betrayal_title: String,
    pub betrayal_keep: String,
    pub betrayal_yes: String,
    pub transfer_title: String,
    pub transfer_cancel: String,
    pub transfer_send: String,
    pub transfer_request: String,
    pub reject_all: String,
    pub accept_all: String,
    pub err_need_gold: String,
    pub err_break_alliance_boat: String,
    pub err_resources_allies_only: String,
    pub err_alliance_renewal_pending: String,
    pub err_alliance_request_pending: String,
    pub alliance_requested: String,
    pub build_bunker_title: String,
    pub build_bunker_coverage: String,
    pub build_factory_title: String,
    pub build_factory_gold: String,
    pub build_port_title: String,
    pub build_port_fleet: String,
    pub build_port_troops: String,
    pub build_port_gold: String,
    pub transfer_confirm_title: String,
    pub transfer_confirm_body: String,
    pub transfer_confirm_yes: String,
    pub confirm_exit_title: String,
    pub confirm_exit_body: String,
    pub confirm_exit_yes: String,
    pub confirm_exit_no: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[cfg(feature = "map-editor")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapEditorStrings {
    pub title: String,
    pub btn_new: String,
    pub btn_export: String,
    pub btn_exit: String,
    pub btn_from_osm: String,
    pub btn_back_to_brush: String,
    pub btn_generate_osm: String,
    pub label_size: String,
    pub label_map_name: String,
    pub label_map_slug_hint: String,
    pub label_osm_zoom: String,
    pub label_osm_center: String,
    pub label_osm_target: String,
    pub heading_osm: String,
    pub heading_tools: String,
    pub instructions_osm: String,
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
    pub heading_npcs: String,
    pub btn_place_spawn: String,
    pub label_placed_spawns: String,
    pub hover_nation_name: String,
    pub hover_flag: String,
    pub win_new_title: String,
    pub label_width: String,
    pub label_height: String,
    pub btn_create_map: String,
    pub msg_blank_created: String,
    pub msg_spawn_placed: String,
    pub msg_spawn_removed: String,
    pub msg_compiling: String,
    pub msg_saved: String,
    pub msg_saved_sp: String,
    pub msg_saved_download: String,
    pub msg_write_failed: String,
    pub btn_export_download: String,
    pub msg_osm_generating: String,
    pub msg_osm_classifying: String,
    pub msg_osm_generated: String,
    pub msg_osm_failed: String,
    pub msg_osm_no_selection: String,
    pub msg_osm_selection_too_large: String,
    pub label_osm_bbox_sw: String,
    pub label_osm_bbox_ne: String,
    pub label_osm_overpass_tiles: String,
    pub hint_osm_overpass_limit: String,
    pub osm_attribution: String,
    pub btn_cancel: String,
    pub btn_undo: String,
    pub confirm_exit_title: String,
    pub confirm_exit_body: String,
    pub confirm_export_title: String,
    pub confirm_export_body: String,
    pub confirm_yes: String,
    pub confirm_no: String,
    pub hint_shortcuts: String,
    pub tooltip_undo: String,
    pub tooltip_back_to_brush: String,
    pub tooltip_export: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditsStrings {
    pub title: String,
    pub link: String,
    pub close: String,
    pub based_on_short: String,
    pub copyright_sow: String,
    pub license_agpl: String,
    pub based_on: String,
    pub assets_license: String,
    pub source_label: String,
    pub privacy_label: String,
    pub notice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalLink {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalSection {
    pub heading: String,
    #[serde(default)]
    pub paragraphs: Vec<String>,
    #[serde(default)]
    pub bullets: Vec<String>,
    #[serde(default)]
    pub links: Vec<LegalLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalDocument {
    pub title: String,
    pub updated: String,
    pub close: String,
    pub sections: Vec<LegalSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageStrings {
    pub main_menu: MainMenuStrings,
    pub settings: SettingsStrings,
    pub loading_screen: LoadingScreenStrings,
    pub endgame: EndgameStrings,
    pub hud: HudStrings,
    pub tutorial: TutorialStrings,
    #[cfg(feature = "map-editor")]
    pub map_editor: MapEditorStrings,
    pub credits: CreditsStrings,
}

static EN_STRINGS: OnceLock<LanguageStrings> = OnceLock::new();
static ES_STRINGS: OnceLock<LanguageStrings> = OnceLock::new();
static PRIVACY_EN: OnceLock<LegalDocument> = OnceLock::new();
static TERMS_EN: OnceLock<LegalDocument> = OnceLock::new();

fn parse_legal(toml: &str) -> LegalDocument {
    toml::from_str(toml).expect("Failed to parse legal document TOML")
}

/// Privacy policy (English; embedded from sow-web).
pub fn privacy(_lang: Language) -> &'static LegalDocument {
    PRIVACY_EN.get_or_init(|| parse_legal(include_str!("../../sow-web/site/legal/privacy.en.toml")))
}

/// Terms of service (English; embedded from sow-web).
pub fn terms(_lang: Language) -> &'static LegalDocument {
    TERMS_EN.get_or_init(|| parse_legal(include_str!("../../sow-web/site/legal/terms.en.toml")))
}

#[cfg(not(feature = "map-editor"))]
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
                include_str!("../strings/es/credits.toml"),
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
                include_str!("../strings/en/credits.toml"),
            )
        }),
    }
}

#[cfg(feature = "map-editor")]
pub fn get(lang: Language) -> &'static LanguageStrings {
    match lang {
        Language::Spanish => ES_STRINGS.get_or_init(|| {
            load_language_with_map_editor(I18nLoadCtx {
                main_menu_toml: include_str!("../strings/es/main_menu.toml"),
                settings_toml: include_str!("../strings/es/settings.toml"),
                loading_screen_toml: include_str!("../strings/es/loading_screen.toml"),
                endgame_toml: include_str!("../strings/es/endgame.toml"),
                hud_toml: include_str!("../strings/es/hud.toml"),
                tutorial_toml: include_str!("../strings/es/tutorial.toml"),
                map_editor_toml: include_str!("../strings/es/map_editor.toml"),
                credits_toml: include_str!("../strings/es/credits.toml"),
            })
        }),
        _ => EN_STRINGS.get_or_init(|| {
            load_language_with_map_editor(I18nLoadCtx {
                main_menu_toml: include_str!("../strings/en/main_menu.toml"),
                settings_toml: include_str!("../strings/en/settings.toml"),
                loading_screen_toml: include_str!("../strings/en/loading_screen.toml"),
                endgame_toml: include_str!("../strings/en/endgame.toml"),
                hud_toml: include_str!("../strings/en/hud.toml"),
                tutorial_toml: include_str!("../strings/en/tutorial.toml"),
                map_editor_toml: include_str!("../strings/en/map_editor.toml"),
                credits_toml: include_str!("../strings/en/credits.toml"),
            })
        }),
    }
}

fn default_team_victory_subtitle() -> String {
    "Your team has conquered the world.".to_string()
}

fn default_team_defeat_subtitle() -> String {
    "Your team has fallen.".to_string()
}

fn default_team_winner_emerged() -> String {
    "Team {} emerged victorious.".to_string()
}

#[cfg(not(feature = "map-editor"))]
fn load_language(
    main_menu_toml: &str,
    settings_toml: &str,
    loading_screen_toml: &str,
    endgame_toml: &str,
    hud_toml: &str,
    tutorial_toml: &str,
    credits_toml: &str,
) -> LanguageStrings {
    LanguageStrings {
        main_menu: toml::from_str(main_menu_toml).expect("Failed to parse main_menu.toml"),
        settings: toml::from_str(settings_toml).expect("Failed to parse settings.toml"),
        loading_screen: toml::from_str(loading_screen_toml)
            .expect("Failed to parse loading_screen.toml"),
        endgame: toml::from_str(endgame_toml).expect("Failed to parse endgame.toml"),
        hud: toml::from_str(hud_toml).expect("Failed to parse hud.toml"),
        tutorial: toml::from_str(tutorial_toml).expect("Failed to parse tutorial.toml"),
        credits: toml::from_str(credits_toml).expect("Failed to parse credits.toml"),
    }
}

#[cfg(feature = "map-editor")]
struct I18nLoadCtx<'a> {
    main_menu_toml: &'a str,
    settings_toml: &'a str,
    loading_screen_toml: &'a str,
    endgame_toml: &'a str,
    hud_toml: &'a str,
    tutorial_toml: &'a str,
    map_editor_toml: &'a str,
    credits_toml: &'a str,
}

#[cfg(feature = "map-editor")]
fn load_language_with_map_editor(ctx: I18nLoadCtx) -> LanguageStrings {
    LanguageStrings {
        main_menu: toml::from_str(ctx.main_menu_toml).expect("Failed to parse main_menu.toml"),
        settings: toml::from_str(ctx.settings_toml).expect("Failed to parse settings.toml"),
        loading_screen: toml::from_str(ctx.loading_screen_toml)
            .expect("Failed to parse loading_screen.toml"),
        endgame: toml::from_str(ctx.endgame_toml).expect("Failed to parse endgame.toml"),
        hud: toml::from_str(ctx.hud_toml).expect("Failed to parse hud.toml"),
        tutorial: toml::from_str(ctx.tutorial_toml).expect("Failed to parse tutorial.toml"),
        map_editor: toml::from_str(ctx.map_editor_toml).expect("Failed to parse map_editor.toml"),
        credits: toml::from_str(ctx.credits_toml).expect("Failed to parse credits.toml"),
    }
}
