use std::sync::Arc;
use web_time::{Duration, Instant};

/// Subset of [`sow_core::protocol::ProjectileSnapshot`] for detonation / launch detection.
#[derive(Clone, Copy, Debug)]
pub struct TrackedProjectile {
    pub kind: sow_core::game::ProjectileKind,
    pub dst_tile: u32,
    pub path_cursor: usize,
    pub steps_per_tick: u8,
    pub path_len: usize,
}

impl TrackedProjectile {
    pub fn from_snapshot(proj: &sow_core::protocol::ProjectileSnapshot) -> Self {
        Self {
            kind: proj.kind,
            dst_tile: proj.dst_tile,
            path_cursor: proj.path_cursor,
            steps_per_tick: proj.steps_per_tick,
            path_len: proj.path.len(),
        }
    }

    pub fn at_path_end(&self) -> bool {
        self.path_cursor + self.steps_per_tick as usize >= self.path_len
    }
}

/// Cached nameplate text layouts — rebuilt only when name, font, or troops change.
pub struct CachedNameplate {
    pub display_name: String,
    pub troops_str: String,
    pub font_id: egui::FontId,
    pub prepared_name: sow_ui::widgets::PreparedName,
    pub troops_galley: Arc<egui::Galley>,
}

pub struct GraphicsState {
    pub window: Option<Box<dyn winit::window::Window>>,
    pub surface: Option<blade_graphics::Surface>,
    pub render_ctx: Option<sow_render::RenderContext>,
    pub map_renderer: Option<sow_render::MapRenderer>,
    pub mover_renderer: Option<sow_render::MoverRenderer>,
    pub gui_painter: Option<blade_egui::GuiPainter>,
    pub prev_sync_point: Option<blade_graphics::SyncPoint>,
    pub needs_first_upload: bool,
    pub configured_physical: winit::dpi::PhysicalSize<u32>,
    /// Deferred teardown after instant exit (must not run mid-frame during UI actions).
    pub pending_session_cleanup: bool,
    /// Last viewport applied to egui (`physical_w`, `physical_h`, `scale_factor`).
    pub last_egui_viewport: Option<(u32, u32, f32)>,
}

pub struct NetState {
    pub client: Option<sow_net::client::SowClient>,
    pub connect_tx: crossbeam_channel::Sender<Result<sow_net::client::SowClient, String>>,
    pub connect_rx: crossbeam_channel::Receiver<Result<sow_net::client::SowClient, String>>,
    pub ws_url: String,
    pub orchestrator_url: String,
    pub is_offline: bool,
    pub ws_connect_fail_backoff_ms: u64,
    pub ws_connect_not_before: web_time::Instant,
    pub ws_reconnect_after_resume: bool,
    pub pending_lobby_rejoin: bool,
    pub current_ping_ms: Option<u32>,
    pub last_ping_time: web_time::Instant,
    pub relay_connect_start: Option<web_time::Instant>,
    pub relay_retry_count: u32,
}

pub struct SimState {
    pub engine: Option<sow_core::engine::SowEngine>,
    pub current_snapshot: Option<sow_core::protocol::SimSnapshot>,
    pub turn_queue: std::collections::VecDeque<sow_core::protocol::Turn>,
    pub my_player_id: Option<u16>,
    pub my_lobby_id: Option<u64>,
    pub map_w: u32,
    pub map_h: u32,
    pub offline_tick_timer: f32,
    /// Wall-clock anchor for offline sim pacing (decoupled from render interp).
    pub offline_last_update: web_time::Instant,
    pub offline_intents: Vec<sow_core::protocol::GameplayIntent>,
    pub last_synced_cost_tick: Option<u64>,
    pub tile_upgrades: Vec<u32>,
    pub config: sow_core::game_config::GameConfig,
}

pub struct InputState {
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub target_zoom: f32,
    pub screen_w: f32,
    pub screen_h: f32,
    pub dragging: bool,
    pub last_mouse_x: f64,
    pub last_mouse_y: f64,
    pub active_touches: std::collections::HashMap<u64, (f64, f64)>,
    pub map_touch_start: Option<(web_time::Instant, f64, f64)>,
    pub map_context_menu: Option<(f32, f32, u32)>,
    pub map_context_menu_active: Option<(f32, f32, u32)>,
    pub map_context_menu_session: u64,
    pub context_menu_timer: f32,
    pub context_menu_open_time: Option<web_time::Instant>,
    pub last_pinch_state: Option<(f64, f64, f64)>,
    /// Hold-to-attack: (target_owner, press_start_time, screen_x, screen_y, has_fired_initial)
    pub hold_attack_target: Option<(u16, web_time::Instant, f64, f64, bool)>,
    pub hold_attack_accum: f32,
    pub ime_allowed_state: bool,
    pub ime_cursor_rect_px: Option<egui::Rect>,
    pub has_snapped_camera_to_spawn: bool,
    pub selected_warships: Vec<u64>,
}

#[derive(Clone, Debug)]
pub struct FalloutZone {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub start_time: web_time::Instant,
}

#[derive(Clone, Debug)]
pub struct ActiveUpgradeAnimation {
    pub tile_idx: u32,
    pub start_time: web_time::Instant,
    pub duration: web_time::Duration,
    pub kind: sow_core::game::BuildingKind,
    pub level: u8,
}

#[derive(Clone, Debug)]
pub struct FloatingNotice {
    pub text: String,
    pub world_x: f32,
    pub world_y: f32,
    pub start_time: web_time::Instant,
    pub duration: web_time::Duration,
    pub color: egui::Color32,
}

#[derive(Clone, Debug)]
pub struct DeathNameplateAnimation {
    pub name: String,
    pub color: egui::Color32,
    pub world_x: f32,
    pub world_y: f32,
    pub start_time: web_time::Instant,
    pub duration: web_time::Duration,
    pub seed: u32,
    pub player_type: sow_core::player::PlayerType,
    pub player_id: u16,
    pub nameplate_size: f32,
    pub by_nuke: bool,
}

#[derive(Clone, Debug)]
pub struct ClickMarker {
    pub world_x: f32,
    pub world_y: f32,
    pub start_time: web_time::Instant,
}

#[allow(clippy::type_complexity)]
pub struct UiState {
    pub app: sow_ui::ClientApp,
    pub egui_ctx: egui::Context,
    pub raw_input: egui::RawInput,

    pub label_positions: std::collections::HashMap<u16, (f32, f32)>,
    pub label_sizes: std::collections::HashMap<u16, f32>,
    /// True while the portal intro or manual offline tutorial overlay is active.
    pub tutorial_active: bool,
    pub tutorial_step: crate::hud::tutorial::TutorialStep,
    pub show_leaderboard: bool,
    pub leaderboard_timer: f32,
    pub leaderboard_rankings: Vec<crate::hud::leaderboard::LeaderboardRanking>,
    pub leaderboard_display:
        std::collections::HashMap<u16, crate::hud::leaderboard::LeaderboardRowDisplay>,
    pub leaderboard_visible_limit: usize,
    pub leaderboard_paged_through_limit: usize,
    pub leaderboard_search: String,
    pub leaderboard_team_rankings: Vec<crate::hud::leaderboard::TeamRanking>,
    pub leaderboard_prev_search: String,
    pub leaderboard_was_open: bool,
    pub show_dev_sidebar: bool,
    pub update_available: bool,
    pub is_spectating: bool,
    pub fallout_zones: Vec<FalloutZone>,
    pub last_projectiles: std::collections::HashMap<u64, TrackedProjectile>,
    pub active_upgrades: Vec<ActiveUpgradeAnimation>,
    pub nameplate_galleys: std::collections::HashMap<u16, CachedNameplate>,
    pub nameplate_troops_last_update: std::collections::HashMap<u16, web_time::Instant>,
    pub cached_player_colors: Vec<egui::Color32>,
    pub cached_player_count: usize,
    pub star_svg_registered: bool,
    pub floating_notices: Vec<FloatingNotice>,
    pub death_nameplates: Vec<DeathNameplateAnimation>,
    /// Cached endgame copy for panel fade-out (is_victory, title, subtitle).
    pub endgame_cache: Option<(bool, String, String)>,

    pub cached_hovered_building_id: Option<u64>,
    pub cached_hovered_building_level: u8,
    pub cached_hovered_building_tooltip: String,
    pub attack_troop_labels: std::collections::HashMap<u64, (f64, String)>,
    pub edge_mask_cache: Vec<u8>,
    pub rail_state: crate::render::world::railways::RailState,
    /// Client-side nuke silo cooldown tracking: building id → tick when ready.
    pub silo_cooldowns: std::collections::HashMap<u64, u64>,
    /// Last sim tick copied into `hud_state` combat vecs.
    pub hud_combat_sync_tick: u64,
    pub bunker_last_sound_time: std::collections::HashMap<u64, web_time::Instant>,
    pub mover_scene: crate::render::world::movers::MoverScene,
    pub click_markers: Vec<ClickMarker>,
}

impl UiState {
    pub fn invalidate_egui_dependent_caches(&mut self) {
        self.nameplate_galleys.clear();
        self.nameplate_troops_last_update.clear();
    }
}

/// Wall-clock anchor for render-behind-by-one-tick interpolation between sim snapshots.
pub struct InterpClock {
    pub last_applied_at: web_time::Instant,
    pub tick_dur: Duration,
}

impl InterpClock {
    #[inline]
    pub fn alpha(&self, now: Instant) -> f32 {
        let elapsed = now.duration_since(self.last_applied_at).as_secs_f32();
        let dur = self.tick_dur.as_secs_f32().max(0.001);
        let t = (elapsed / dur).clamp(0.0, 1.0);
        // Smoothstep — same feel as legacy fleet/nuke overlays.
        t * t * (3.0 - 2.0 * t)
    }

    pub fn stamp_applied(&mut self, now: Instant) {
        self.last_applied_at = now;
    }

    pub fn set_tick_dur_ms(&mut self, tick_rate_ms: f32) {
        self.tick_dur = Duration::from_secs_f32((tick_rate_ms / 1000.0).max(0.001));
    }
}

pub struct TimeState {
    pub interp: InterpClock,
    pub start_time: web_time::Instant,
    pub frame_count: u32,
    pub last_fps_time: web_time::Instant,
    pub current_fps: u32,
    pub last_frame_time: web_time::Instant,
    pub last_debug_print: Option<web_time::Instant>,
}

pub struct TaskState {
    pub map_tx: crossbeam_channel::Sender<crate::MapDownloadEvent>,
    pub map_rx: crossbeam_channel::Receiver<crate::MapDownloadEvent>,
    pub engine_init_tx: crossbeam_channel::Sender<crate::EngineInitEvent>,
    pub engine_init_rx: crossbeam_channel::Receiver<crate::EngineInitEvent>,
    pub db_tx: crossbeam_channel::Sender<crate::player_progress::DbEvent>,
    pub db_rx: crossbeam_channel::Receiver<crate::player_progress::DbEvent>,
    pub pending_engine_init_data: Option<(
        sow_core::game::GameState,
        sow_core::water_components::WaterComponents,
        sow_core::protocol::ServerStartMessage,
    )>,
    pub engine_init_queued_msg: Option<sow_core::protocol::ServerStartMessage>,
}

pub struct SowApp {
    pub gfx: GraphicsState,
    pub net: NetState,
    pub sim: SimState,
    pub input: InputState,
    pub ui: UiState,
    pub time: TimeState,
    pub tasks: TaskState,

    #[cfg(not(target_arch = "wasm32"))]
    pub tokio_rt: tokio::runtime::Runtime,
    #[cfg(target_arch = "wasm32")]
    pub wasm_doc_was_visible: bool,
    pub asset_config: crate::AssetConfig,
    #[cfg(target_arch = "wasm32")]
    pub(crate) web_loader_hidden: bool,
    #[cfg(target_arch = "wasm32")]
    pub(crate) ime_bridge: crate::ime::WasmImeBridge,
    /// Set when Blade/Vulkan init fails; event loop exits on next tick.
    pub gpu_init_failed: bool,
    pub progress: crate::player_progress::PlayerProgress,
    pub progress_account_id: Option<String>,
    pub progress_provider: String,
    pub progress_match_recorded: bool,
    pub progress_stats_submitted: bool,
    pub progress_session_defeats: crate::player_progress::SessionDefeats,
    #[cfg(target_arch = "wasm32")]
    pub boot_db_settled: bool,
    #[cfg(target_arch = "wasm32")]
    pub boot_route_waiting: bool,
    #[cfg(target_arch = "wasm32")]
    pub boot_ready_since: Option<web_time::Instant>,
}
