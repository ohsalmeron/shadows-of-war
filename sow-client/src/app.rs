use sow_render::{MapRenderer, RenderContext};

use crate::{get_build_version, spawn_sow_client_connect};
use crate::{EngineInitEvent, MapDownloadEvent};
use blade_egui::GuiPainter;
use blade_graphics as gpu;
use egui::{Context, RawInput, Rect};
use sow_core::protocol::SimSnapshot;
use sow_net::client::SowClient;
use sow_ui::{app::ClientPhase, ClientApp};
use std::collections::HashMap;
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

impl Default for SowApp {
    fn default() -> Self {
        Self::new()
    }
}

impl SowApp {
    pub fn new() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = env_logger::builder()
                .filter_level(log::LevelFilter::Info)
                .try_init();
        }
        // ── Simulation ──────────────────────────────────────────────────────────
        let map_w: u32 = 800;
        let map_h: u32 = 600;

        let engine: Option<sow_core::engine::SowEngine> = None;
        // Sim stays idle until a real `SimCommand::Init` (EnterGame or ExitGame cleanup).
        // Eager Init here duplicated the whole map sim at startup and doubled worker snapshots.

        let current_snapshot: Option<SimSnapshot> = None;

        // ── Renderer ────────────────────────────────────────────────────────────
        let render_ctx: Option<RenderContext> = None;
        let surface: Option<gpu::Surface> = None;
        let map_renderer: Option<MapRenderer> = None;
        let mover_renderer: Option<sow_render::MoverRenderer> = None;
        let gui_painter: Option<GuiPainter> = None;
        let window: Option<Box<dyn winit::window::Window>> = None;

        // ── UI State ────────────────────────────────────────────────────────────
        let asset_config = crate::AssetConfig::resolve();
        let mut app = ClientApp::new();
        crate::map_cache::hydrate_asset_maps(&mut app.asset_loader.maps);
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(mql)) = window.match_media("(prefers-reduced-motion: reduce)") {
                    if mql.matches() {
                        app.settings_state.reduced_motion = true;
                    }
                }
            }
        }
        let egui_ctx = Context::default();
        sow_ui::ui::theme::apply_theme(&egui_ctx);
        let raw_input = RawInput::default();

        // ── Network State ───────────────────────────────────────────────────────
        #[cfg(not(target_arch = "wasm32"))]
        let tokio_rt = tokio::runtime::Runtime::new().unwrap();
        let net_client: Option<SowClient> = None;
        let turn_queue = std::collections::VecDeque::new();
        let my_player_id: Option<u16> = None;
        let my_lobby_id: Option<u64> = None;
        let (map_tx, map_rx) = crossbeam_channel::unbounded::<MapDownloadEvent>();
        let (db_tx, db_rx) = crossbeam_channel::unbounded::<crate::player_progress::DbEvent>();
        type EngineInitData = (
            sow_core::game::GameState,
            sow_core::water_components::WaterComponents,
            sow_core::protocol::ServerStartMessage,
        );
        let (engine_init_tx, engine_init_rx) = crossbeam_channel::unbounded::<EngineInitEvent>();
        let pending_engine_init_data: Option<EngineInitData> = None;
        let engine_init_queued_msg: Option<sow_core::protocol::ServerStartMessage> = None;

        let (connect_tx, connect_rx) = crossbeam_channel::unbounded();

        // Reconnect scheduling (idle drop / resume / failed handshake).
        let ws_connect_fail_backoff_ms: u64 = 400;
        let ws_connect_not_before: Instant = Instant::now();
        let ws_reconnect_after_resume: bool = false;
        #[cfg(target_arch = "wasm32")]
        let wasm_doc_was_visible: bool = true;
        #[cfg(target_arch = "wasm32")]
        let web_loader_hidden: bool = false;
        #[cfg(target_arch = "wasm32")]
        let ime_bridge = crate::ime::WasmImeBridge::new();

        #[allow(unused_mut)]
        let mut ws_url =
            std::env::var("SOW_WS_URL").unwrap_or_else(|_| "wss://shadowsofwar.io/ws/".to_string());
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                let mut found_in_js = false;
                if let Ok(val) =
                    js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("SOW_WS_URL"))
                {
                    if let Some(s) = val.as_string() {
                        ws_url = s;
                        found_in_js = true;
                    }
                }
                if !found_in_js {
                    if let Ok(host) = window.location().host() {
                        let protocol =
                            if window.location().protocol().unwrap_or_default() == "https:" {
                                "wss"
                            } else {
                                "ws"
                            };
                        ws_url = format!("{}://{}/ws/", protocol, host);
                    }
                }
            }
        }
        app.main_menu_state.server_address = ws_url.clone();
        let orchestrator_url = ws_url.clone();

        #[cfg(target_arch = "wasm32")]
        {
            let fallback = app.main_menu_state.player_name.clone();
            let identity = crate::store_portals::load_identity(&fallback);
            app.main_menu_state.player_name = identity.display_name;
            app.main_menu_state.name_locked = identity.name_locked;
            if let Some(id) = crate::store_portals::take_pending_invite_lobby() {
                app.main_menu_state.pending_join_lobby_id = Some(id);
                app.main_menu_state.is_waiting = true;
            }
            if crate::store_portals::take_host_private_pending() {
                app.main_menu_state.host_private_pending = true;
                app.main_menu_state.is_waiting = true;
            }
        }

        log::info!("Auto-connecting to {}...", ws_url);
        app.main_menu_state.is_connecting = true;
        #[cfg(target_arch = "wasm32")]
        spawn_sow_client_connect(ws_url.clone(), &connect_tx);
        #[cfg(not(target_arch = "wasm32"))]
        spawn_sow_client_connect(ws_url.clone(), &connect_tx, &tokio_rt);

        // ── Camera state ────────────────────────────────────────────────────────
        let camera_zoom: f32 = 0.5;
        let camera_x: f32 = 1280.0 * 0.5 - (map_w as f32 * 0.5) * camera_zoom;
        let camera_y: f32 = 720.0 * 0.5 - (map_h as f32 * 0.5) * camera_zoom;
        let screen_w: f32 = 1280.0;
        let screen_h: f32 = 720.0;

        // Mouse drag state
        let dragging = false;
        let last_mouse_x: f64 = 0.0;
        let last_mouse_y: f64 = 0.0;

        // Touch state for pinch-to-zoom
        let active_touches: HashMap<u64, (f64, f64)> = HashMap::new();
        let map_touch_start: Option<(Instant, f64, f64)> = None;
        let map_context_menu: Option<(f32, f32, u32)> = None;
        let last_pinch_state: Option<(f64, f64, f64)> = None;

        // Tracks last `Window::set_ime_allowed` value (mirrors egui-winit debounce).
        let ime_allowed_state = false;
        // Last physical-pixel IME area for `set_ime_cursor_area`, for debouncing.
        let ime_cursor_rect_px: Option<Rect> = None;
        let has_snapped_camera_to_spawn = false;

        let prev_sync_point: Option<gpu::SyncPoint> = None;
        let start_time = Instant::now();
        let interp = InterpClock {
            last_applied_at: start_time,
            tick_dur: Duration::from_millis(100),
        };
        let needs_first_upload = true;

        let frame_count = 0;
        let last_fps_time = Instant::now();
        let current_fps = 0;
        let current_ping_ms: Option<u32> = None;
        let last_ping_time = Instant::now();
        let last_frame_time = Instant::now();

        #[allow(unused_mut)]
        let mut sow_app = Self {
            gfx: GraphicsState {
                window,
                surface,
                render_ctx,
                map_renderer,
                mover_renderer,
                gui_painter,
                prev_sync_point,
                needs_first_upload,
                configured_physical: winit::dpi::PhysicalSize::new(0, 0),
                pending_session_cleanup: false,
                last_egui_viewport: None,
            },
            net: NetState {
                client: net_client,
                connect_tx,
                connect_rx,
                ws_url,
                orchestrator_url,
                is_offline: false,
                ws_connect_fail_backoff_ms,
                ws_connect_not_before,
                ws_reconnect_after_resume,
                pending_lobby_rejoin: false,
                current_ping_ms,
                last_ping_time,
                relay_connect_start: None,
                relay_retry_count: 0,
            },
            sim: SimState {
                engine,
                current_snapshot,
                turn_queue,
                my_player_id,
                my_lobby_id,
                map_w,
                map_h,
                offline_tick_timer: 0.0,
                offline_last_update: web_time::Instant::now(),
                offline_intents: Vec::new(),
                last_synced_cost_tick: None,
                tile_upgrades: Vec::new(),
                config: sow_core::game_config::GameConfig::default(),
            },
            input: InputState {
                camera_x,
                camera_y,
                camera_zoom,
                screen_w,
                screen_h,
                dragging,
                last_mouse_x,
                last_mouse_y,
                active_touches,
                map_touch_start,
                map_context_menu,
                map_context_menu_active: None,
                map_context_menu_session: 0,
                context_menu_timer: 0.0,
                context_menu_open_time: None,
                last_pinch_state,
                hold_attack_target: None,
                hold_attack_accum: 0.0,
                ime_allowed_state,
                ime_cursor_rect_px,
                has_snapped_camera_to_spawn,
                selected_warships: Vec::new(),
            },
            ui: UiState {
                app,
                egui_ctx,
                raw_input,

                label_positions: std::collections::HashMap::new(),
                label_sizes: std::collections::HashMap::new(),
                tutorial_active: false,
                tutorial_step: crate::hud::tutorial::TutorialStep::Welcome,
                show_leaderboard: false,
                leaderboard_timer: 0.0,
                leaderboard_rankings: Vec::new(),
                leaderboard_display: std::collections::HashMap::new(),
                leaderboard_visible_limit: crate::hud::leaderboard::INITIAL_VISIBLE_LIMIT,
                leaderboard_paged_through_limit: 0,
                leaderboard_search: String::new(),
                leaderboard_team_rankings: Vec::new(),
                leaderboard_prev_search: String::new(),
                leaderboard_was_open: false,
                show_dev_sidebar: false,
                update_available: false,
                is_spectating: false,
                fallout_zones: Vec::new(),
                last_projectiles: std::collections::HashMap::new(),
                active_upgrades: Vec::new(),
                nameplate_galleys: std::collections::HashMap::new(),
                nameplate_troops_last_update: std::collections::HashMap::new(),
                cached_player_colors: Vec::new(),
                cached_player_count: 0,
                star_svg_registered: false,
                floating_notices: Vec::new(),
                death_nameplates: Vec::new(),
                endgame_cache: None,

                cached_hovered_building_id: None,
                cached_hovered_building_level: 0,
                cached_hovered_building_tooltip: String::new(),
                attack_troop_labels: std::collections::HashMap::new(),
                edge_mask_cache: Vec::new(),
                rail_state: crate::render::world::railways::RailState::new(),
                silo_cooldowns: std::collections::HashMap::new(),
                hud_combat_sync_tick: 0,
                bunker_last_sound_time: std::collections::HashMap::new(),
                mover_scene: crate::render::world::movers::MoverScene::new(),
                click_markers: Vec::new(),
            },
            time: TimeState {
                interp,
                start_time,
                frame_count,
                last_fps_time,
                current_fps,
                last_frame_time,
                last_debug_print: None,
            },
            tasks: TaskState {
                map_tx,
                map_rx,
                engine_init_tx,
                engine_init_rx,
                db_tx,
                db_rx,
                pending_engine_init_data,
                engine_init_queued_msg,
            },
            #[cfg(not(target_arch = "wasm32"))]
            tokio_rt,
            asset_config,
            #[cfg(target_arch = "wasm32")]
            wasm_doc_was_visible,
            #[cfg(target_arch = "wasm32")]
            web_loader_hidden,
            #[cfg(target_arch = "wasm32")]
            ime_bridge,
            gpu_init_failed: false,
            progress: crate::player_progress::PlayerProgress::default(),
            progress_account_id: None,
            progress_provider: String::from("local"),
            progress_match_recorded: false,
            progress_stats_submitted: false,
            progress_session_defeats: crate::player_progress::SessionDefeats::default(),
            #[cfg(target_arch = "wasm32")]
            boot_db_settled: false,
            #[cfg(target_arch = "wasm32")]
            boot_route_waiting: false,
            #[cfg(target_arch = "wasm32")]
            boot_ready_since: None,
        };
        #[cfg(target_arch = "wasm32")]
        if let Some(portal) = crate::store_portals::load_portal_progress() {
            sow_app.progress = portal;
        }
        #[cfg(target_arch = "wasm32")]
        {
            if crate::store_portals::should_fetch_cloud_profile() {
                sow_app.fetch_cloud_progress();
            } else if crate::store_portals::is_portal_embed() {
                sow_app.boot_db_settled = true;
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        sow_app.fetch_cloud_progress();
        sow_app
    }

    fn apply_platform_auth(request: &mut ehttp::Request) {
        if let Some(token) = crate::store_portals::load_identity("Player")
            .auth_token
            .filter(|t| !t.is_empty())
        {
            request.headers.insert("X-Platform-Auth", token);
        }
    }

    pub(crate) fn fetch_cloud_progress(&self) {
        let (provider, ext_id) = crate::store_portals::database_identity("Player");
        let display_name = crate::store_portals::load_identity("Player").display_name;
        let db_url = self.asset_config.database_base.clone();

        let encoded_provider =
            url::form_urlencoded::byte_serialize(provider.as_bytes()).collect::<String>();
        let encoded_id =
            url::form_urlencoded::byte_serialize(ext_id.as_bytes()).collect::<String>();
        let encoded_name =
            url::form_urlencoded::byte_serialize(display_name.as_bytes()).collect::<String>();

        let url = format!(
            "{}/profile?provider={}&external_id={}&fallback_name={}",
            db_url.trim_end_matches('/'),
            encoded_provider,
            encoded_id,
            encoded_name
        );

        log::info!("Fetching profile from sow-database: {provider}/{ext_id}");
        let tx = self.tasks.db_tx.clone();
        let profile_provider = provider.clone();
        let mut request = ehttp::Request::get(&url);
        Self::apply_platform_auth(&mut request);

        ehttp::fetch(
            request,
            move |result: ehttp::Result<ehttp::Response>| match result {
                Ok(res) => {
                    if res.ok {
                        #[derive(serde::Deserialize)]
                        struct DbAccount {
                            id: String,
                            profile: crate::player_progress::PlayerProgress,
                        }
                        match serde_json::from_slice::<DbAccount>(&res.bytes) {
                            Ok(account) => {
                                let _ = tx.send(crate::player_progress::DbEvent::ProfileLoaded {
                                    progress: account.profile,
                                    account_id: account.id,
                                    provider: profile_provider,
                                });
                            }
                            Err(e) => {
                                log::error!("Failed to parse database profile JSON: {}", e);
                                let _ = tx.send(crate::player_progress::DbEvent::LoadFailed);
                            }
                        }
                    } else {
                        log::warn!("sow-database responded with HTTP {}", res.status);
                        let _ = tx.send(crate::player_progress::DbEvent::LoadFailed);
                    }
                }
                Err(e) => {
                    log::error!("sow-database request failed: {}", e);
                    let _ = tx.send(crate::player_progress::DbEvent::LoadFailed);
                }
            },
        );
    }

    pub(crate) fn resolve_link_conflict(&self, keep_account_id: String) {
        let Some(conflict) = self.ui.app.main_menu_state.active_conflict.clone() else {
            return;
        };

        let db_url = self.asset_config.database_base.clone();
        let url = format!("{}/profile/link/resolve", db_url.trim_end_matches('/'));
        #[derive(serde::Serialize)]
        struct ResolveRequest {
            account_id: String,
            keep_account_id: String,
            target_provider: String,
            target_external_id: String,
        }
        let resolved_provider = conflict.target_provider.clone();
        let payload = ResolveRequest {
            account_id: conflict.current_account_id,
            keep_account_id,
            target_provider: resolved_provider.clone(),
            target_external_id: conflict.target_external_id,
        };
        let Ok(body) = serde_json::to_vec(&payload) else {
            return;
        };
        let tx = self.tasks.db_tx.clone();
        let mut request = ehttp::Request::post(&url, body);
        request.headers.insert("Content-Type", "application/json");
        Self::apply_platform_auth(&mut request);
        log::info!("Resolving platform link conflict...");
        ehttp::fetch(request, move |result| {
            let Ok(res) = result else {
                log::error!("Link resolve request failed");
                return;
            };
            if !res.ok {
                log::warn!("Profile link resolve returned HTTP {}", res.status);
                return;
            }
            #[derive(serde::Deserialize)]
            struct DbAccount {
                id: String,
                profile: crate::player_progress::PlayerProgress,
            }
            #[derive(serde::Deserialize)]
            struct ResolveResponse {
                status: String,
                account: Option<DbAccount>,
            }
            let Ok(parsed) = serde_json::from_slice::<ResolveResponse>(&res.bytes) else {
                log::error!("Failed to parse link resolve response");
                return;
            };
            if parsed.status == "resolved" {
                if let Some(account) = parsed.account {
                    let _ = tx.send(crate::player_progress::DbEvent::LinkResolved {
                        progress: account.profile,
                        account_id: account.id,
                        provider: resolved_provider,
                    });
                }
            }
        });
    }

    pub(crate) fn maybe_link_platform_identity(&self) {
        let Some(account_id) = &self.progress_account_id else {
            return;
        };
        let platform = crate::store_portals::load_identity("Player");
        let Some(ext_id) = platform.external_id.filter(|s| !s.is_empty()) else {
            return;
        };
        if platform.provider == "local" || platform.provider == "self" {
            return;
        }

        let db_url = self.asset_config.database_base.clone();
        let url = format!("{}/profile/link", db_url.trim_end_matches('/'));
        #[derive(serde::Serialize)]
        struct LinkRequest {
            account_id: String,
            target_provider: String,
            target_external_id: String,
        }
        let current_account_id = account_id.clone();
        let target_provider = platform.provider.to_string();
        let target_external_id = ext_id.clone();
        let payload = LinkRequest {
            account_id: current_account_id.clone(),
            target_provider: target_provider.clone(),
            target_external_id: target_external_id.clone(),
        };
        let Ok(body) = serde_json::to_vec(&payload) else {
            return;
        };
        let tx = self.tasks.db_tx.clone();
        let mut request = ehttp::Request::post(&url, body);
        request.headers.insert("Content-Type", "application/json");
        Self::apply_platform_auth(&mut request);
        log::info!(
            "Attempting to link platform {} to account {}",
            platform.provider,
            account_id
        );
        ehttp::fetch(request, move |result| {
            let Ok(res) = result else { return };
            if !res.ok {
                log::warn!("Profile link returned HTTP {}", res.status);
                return;
            }
            #[derive(serde::Deserialize)]
            struct LinkSummary {
                level: u32,
            }
            #[derive(serde::Deserialize)]
            struct LinkResponse {
                status: String,
                existing_account_id: Option<String>,
                existing: Option<LinkSummary>,
                current: Option<LinkSummary>,
            }
            let Ok(parsed) = serde_json::from_slice::<LinkResponse>(&res.bytes) else {
                return;
            };
            match parsed.status.as_str() {
                "linked" | "resolved" => {
                    log::info!("Platform identity linked successfully");
                }
                "conflict" => {
                    if let (Some(existing_id), Some(existing), Some(current)) =
                        (parsed.existing_account_id, parsed.existing, parsed.current)
                    {
                        log::warn!(
                            "Account link conflict: local level {} vs existing level {} (id {})",
                            current.level,
                            existing.level,
                            existing_id
                        );
                        let _ = tx.send(crate::player_progress::DbEvent::LinkConflict(
                            crate::player_progress::LinkConflictInfo {
                                current_account_id: current_account_id.clone(),
                                existing_account_id: existing_id,
                                existing_level: existing.level,
                                current_level: current.level,
                                target_provider,
                                target_external_id,
                            },
                        ));
                    }
                }
                other => log::warn!("Unexpected link status: {other}"),
            }
        });
    }

    pub(crate) fn apply_progress_preferences(&mut self) {
        if let Some(leader) = self.progress.preferred_leader {
            self.ui.app.main_menu_state.selected_leader = leader;
            self.ui.app.main_menu_state.selected_civilization = leader.civilization();
        }
    }

    pub(crate) fn apply_cloud_profile(
        &mut self,
        cloud: crate::player_progress::PlayerProgress,
        account_id: String,
        provider: String,
    ) {
        let portal = self.progress.clone();
        self.progress.merge_boot_profile(cloud);
        self.progress_account_id = Some(account_id);
        self.progress_provider = provider;
        if !self.progress.has_history() && portal.has_history() {
            self.progress = portal;
        }
        self.apply_progress_preferences();
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn should_portal_auto_intro(&self) -> bool {
        if !crate::store_portals::is_portal_embed() {
            return false;
        }
        let mm = &self.ui.app.main_menu_state;
        if self.progress.is_first_game() {
            // A real invite link should still bypass the intro, but instant-MP host intent should not.
            mm.pending_join_lobby_id.is_none()
        } else {
            mm.pending_join_lobby_id.is_none() && !mm.host_private_pending
        }
    }

    pub(crate) fn save_local_progress(&self) {
        crate::store_portals::save_portal_progress(&self.progress);
    }

    fn reset_progress_session(&mut self) {
        self.progress_match_recorded = false;
        self.progress_stats_submitted = false;
        self.progress_session_defeats = crate::player_progress::SessionDefeats::default();
    }

    fn maybe_submit_online_stats(&mut self, snap: &sow_core::protocol::SimSnapshot) {
        if self.progress_stats_submitted {
            return;
        }
        if self.progress_account_id.is_none() || self.net.is_offline {
            return;
        }
        let my_id = self.sim.my_player_id.unwrap_or(0);
        if my_id == 0 {
            return;
        }
        let Some(me) = snap.players.iter().find(|p| p.id == my_id) else {
            return;
        };
        let game_over = snap.winner.is_some();
        let eliminated = !me.alive && me.has_spawned;
        if !game_over && !eliminated {
            return;
        }
        self.progress_stats_submitted = true;

        let msg = sow_core::protocol::ClientMessage::SubmitStats {
            kills: me.kills,
            deaths: me.deaths,
            assists: me.assists,
            players_defeated: self.progress_session_defeats.players,
            empires_defeated: self.progress_session_defeats.empires,
            tribes_defeated: self.progress_session_defeats.tribes,
        };
        if let Ok(json) = bincode::serialize(&msg) {
            if let Some(c) = self.net.client.as_ref() {
                c.send(json);
                log::info!(
                    "Submitted online stats: K/D/A {}/{}/{}",
                    me.kills,
                    me.deaths,
                    me.assists
                );
            }
        }
    }

    fn maybe_record_match_progress(
        &mut self,
        winner: Option<u16>,
        winning_team: Option<sow_core::protocol::Team>,
        my_team: Option<sow_core::protocol::Team>,
    ) {
        if self.progress_match_recorded {
            return;
        }
        let Some(winner_id) = winner else {
            return;
        };
        let my_id = self.sim.my_player_id.unwrap_or(0);
        if my_id == 0 {
            return;
        }
        self.progress_match_recorded = true;

        // Online ranked matches: relay + sow-database own the outcome; client only reads profile later.
        if self.progress_account_id.is_some() && !self.net.is_offline {
            log::info!("Online match ended (winner={winner_id}); stats will sync from sow-database on menu return");
            return;
        }

        let won = if let Some(team) = winning_team {
            my_team == Some(team)
        } else {
            winner_id == my_id
        };
        let defeats = self.progress_session_defeats;
        let (kills, deaths, assists) = self
            .sim
            .current_snapshot
            .as_ref()
            .and_then(|s| s.players.iter().find(|p| p.id == my_id))
            .map(|p| (p.kills, p.deaths, p.assists))
            .unwrap_or((0, 0, 0));
        self.progress.preferred_leader = Some(self.ui.app.main_menu_state.selected_leader);
        self.progress
            .record_match_with_kda(won, defeats, kills, deaths, assists);
        self.save_local_progress();
        crate::store_portals::submit_leaderboard_score(self.progress.xp);
        log::info!(
            "Recorded local match progress: won={won}, defeats={defeats:?}, level={}",
            self.progress.level
        );
    }

    /// Initialize the shared Blade context once; returns false after a fatal error.
    pub fn ensure_render_ctx(&mut self) -> bool {
        if self.gfx.render_ctx.is_some() {
            return true;
        }
        if self.gpu_init_failed {
            return false;
        }
        match RenderContext::try_new() {
            Ok(ctx) => {
                self.gfx.render_ctx = Some(ctx);
                true
            }
            Err(err) => {
                self.gpu_init_failed = true;
                eprintln!(
                    "Failed to initialize GPU (Vulkan).\n\
                     On Linux, ensure Vulkan drivers are installed and loaded.\n\
                     If you use NVIDIA, run `nvidia-smi` — a driver/library version mismatch \
                     requires a reboot after updating nvidia-utils.\n\
                     Close other GPU apps if video memory is exhausted.\n\
                     Details: {err}"
                );
                log::error!("GPU init failed: {err}");
                false
            }
        }
    }

    pub(crate) fn make_join_message(
        &self,
        target_lobby_id: Option<u64>,
        host_private: bool,
    ) -> sow_core::protocol::ClientMessage {
        sow_core::protocol::ClientMessage::Join {
            name: self.ui.app.main_menu_state.player_name.clone(),
            is_observer: false,
            target_lobby_id,
            host_private,
            build_version: get_build_version(),
            clan_tag: self.ui.app.main_menu_state.clan_tag.clone(),
            civilization: self.ui.app.main_menu_state.selected_civilization,
            leader: self.ui.app.main_menu_state.selected_leader,
            database_account_id: self.progress_account_id.clone(),
        }
    }

    pub(crate) fn sync_portal_room(&self, joinable: bool) {
        if let Some(id) = self.ui.app.main_menu_state.joined_lobby_id {
            crate::store_portals::update_room(id, joinable, &get_build_version());
        } else if !joinable {
            crate::store_portals::left_room();
        }
    }

    /// Tear down an online match and run the existing ExitGame splash → MainMenu flow.
    pub(crate) fn begin_exit_to_main_menu(&mut self, use_loader: bool) {
        let was_playing = self.ui.app.phase == sow_ui::app::ClientPhase::Playing;
        if was_playing {
            crate::store_portals::gameplay_stop();
        }
        crate::store_portals::left_room();
        self.net.is_offline = false;
        self.net.ws_url = self.net.orchestrator_url.clone();
        self.ui.app.main_menu_state.server_address = self.net.ws_url.clone();

        // Drop relay connection and force orchestrator reconnect
        self.net.client = None;
        self.ui.app.main_menu_state.is_connected = false;
        self.ui.app.main_menu_state.is_connecting = false;
        while self.net.connect_rx.try_recv().is_ok() {}
        self.net.ws_connect_not_before = web_time::Instant::now();

        self.ui.app.main_menu_state.is_waiting = false;
        self.ui.app.main_menu_state.pending_join_lobby_id = None;
        self.ui.app.main_menu_state.joined_lobby_id = None;
        self.ui.app.hud_state.sync_state = None;
        self.sim.my_lobby_id = None;
        self.sim.my_player_id = None;
        if use_loader {
            self.ui.app.phase = ClientPhase::Splash;
            let lang = self.ui.app.settings_state.language;
            self.ui
                .app
                .splash_state
                .reset_anim(sow_ui::ui::loading_screen::SplashJob::ExitGame, lang);
        } else {
            self.ui.app.phase = ClientPhase::MainMenu;
            if was_playing {
                self.gfx.pending_session_cleanup = true;
            }
        }
        self.ui.is_spectating = false;
        self.ui.endgame_cache = None;
        self.reset_progress_session();

        if was_playing
            && self.progress_account_id.is_some()
            && crate::store_portals::should_fetch_cloud_profile()
        {
            self.fetch_cloud_progress();
        }
    }

    /// Enter the EnterGame splash (fade-in, progress bar, fade-out to Playing).
    pub(crate) fn begin_enter_game_loader(&mut self) {
        self.ui.app.phase = sow_ui::app::ClientPhase::Splash;
        let lang = self.ui.app.settings_state.language;
        self.ui
            .app
            .splash_state
            .reset_anim(sow_ui::ui::loading_screen::SplashJob::EnterGame, lang);
    }

    /// Whether the map/mover GPU path should paint this frame (hidden during splash loads).
    pub(crate) fn should_draw_world(&self) -> bool {
        use sow_ui::app::ClientPhase;
        use sow_ui::ui::loading_screen::SplashJob;

        match self.ui.app.phase {
            ClientPhase::Playing => true,
            ClientPhase::Splash => {
                let s = &self.ui.app.splash_state;
                matches!(s.job, SplashJob::EnterGame) && s.done && s.fadeout_start.is_some()
            }
            ClientPhase::MainMenu => false,
        }
    }

    #[inline]
    pub(crate) fn ws_on_relay(&self) -> bool {
        self.net.ws_url.contains("/relay/") || self.net.ws_url.contains("2557")
    }

    /// Window used for input/redraw.
    pub fn active_window(&self) -> Option<&dyn winit::window::Window> {
        self.gfx.window.as_deref()
    }

    pub fn handle_suspended(&mut self, _event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        let Some(render_ctx) = self.gfx.render_ctx.as_mut() else {
            return;
        };
        if let Some(sp) = self.gfx.prev_sync_point.take() {
            let _ = render_ctx.context.wait_for(&sp, !0);
        }
        if let Some(mut s) = self.gfx.surface.take() {
            if let Some(mut gp) = self.gfx.gui_painter.take() {
                gp.destroy(&render_ctx.context);
            }
            if let Some(mut mr) = self.gfx.map_renderer.take() {
                mr.destroy(render_ctx);
            }
            if let Some(mut mover) = self.gfx.mover_renderer.take() {
                mover.destroy(render_ctx);
            }
            render_ctx.context.destroy_surface(&mut s);
        }
    }

    pub fn handle_resumed(&mut self, event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        // App or tab foregrounded — retry WS soon if the socket died in the background.
        self.net.ws_reconnect_after_resume = true;
        if !self.ensure_render_ctx() {
            event_loop.exit();
            return;
        }
        if self.gfx.window.is_none() {
            #[cfg(any(target_os = "android", target_os = "ios"))]
            #[allow(unused_mut)]
            let mut attributes =
                winit::window::WindowAttributes::default().with_title("Shadows of War");

            #[cfg(target_os = "ios")]
            {
                let ios_attrs = winit::platform::ios::WindowAttributesIos::default()
                    .with_valid_orientations(
                        winit::platform::ios::ValidOrientations::LandscapeAndPortrait,
                    )
                    .with_prefers_status_bar_hidden(true)
                    .with_prefers_home_indicator_hidden(true);
                attributes = attributes.with_platform_attributes(Box::new(ios_attrs));
            }
            #[cfg(target_arch = "wasm32")]
            let mut attributes = {
                let (w, h) = crate::web_canvas::canvas_logical_size();
                winit::window::WindowAttributes::default()
                    .with_title("Shadows of War")
                    .with_surface_size(winit::dpi::LogicalSize::new(w, h))
            };

            #[cfg(target_arch = "wasm32")]
            {
                use wasm_bindgen::JsCast;
                let window = web_sys::window().unwrap();
                let document = window.document().unwrap();
                let canvas = document
                    .get_element_by_id("blade")
                    .unwrap()
                    .dyn_into::<web_sys::HtmlCanvasElement>()
                    .unwrap();
                let web_attrs = winit::platform::web::WindowAttributesWeb::default()
                    .with_canvas(Some(canvas))
                    .with_prevent_default(true);
                attributes = attributes.with_platform_attributes(Box::new(web_attrs));
                crate::ime::ensure_canvas_tabindex();
            }

            #[cfg(not(any(target_os = "android", target_os = "ios", target_family = "wasm")))]
            let attributes = winit::window::WindowAttributes::default()
                .with_title("Shadows of War")
                .with_surface_size(winit::dpi::LogicalSize::new(800.0, 800.0));

            match event_loop.create_window(attributes) {
                Ok(win) => self.gfx.window = Some(win),
                Err(e) => {
                    log::warn!("Window creation failed: {:?}", e);
                    return;
                }
            }
        }
        self.check_surface();
    }
}

impl Drop for SowApp {
    fn drop(&mut self) {
        let Some(render_ctx) = self.gfx.render_ctx.as_mut() else {
            return;
        };
        if let Some(sp) = self.gfx.prev_sync_point.take() {
            let _ = render_ctx.context.wait_for(&sp, !0);
        }
        if let Some(mut mr) = self.gfx.map_renderer.take() {
            mr.destroy(render_ctx);
        }
        if let Some(mut mover) = self.gfx.mover_renderer.take() {
            mover.destroy(render_ctx);
        }
        if let Some(mut gui) = self.gfx.gui_painter.take() {
            gui.destroy(&render_ctx.context);
        }
        if let Some(mut s) = self.gfx.surface.take() {
            render_ctx.context.destroy_surface(&mut s);
        }
        // The command encoder is destroyed by `RenderContext`'s own `Drop`
        // when the `Option<RenderContext>` field is dropped after this.
    }
}

impl SowApp {
    #[cfg_attr(target_arch = "wasm32", allow(unused_variables))]
    pub fn update(&mut self, _event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        self.check_surface();

        let now = web_time::Instant::now();
        self.update_net(now);
        self.update_assets();
        self.update_loader();
        self.update_sim(now);
    }

    pub fn dispatch_sim_command(&mut self, cmd: sow_core::protocol::SimCommand) {
        match cmd {
            sow_core::protocol::SimCommand::Init {
                config,
                seed,
                map_bytes,
                players,
            } => {
                self.reset_progress_session();
                self.sim.config = (*config).clone();
                let map_w = config.map_width;
                let map_h = config.map_height;
                let mut state = sow_core::game::GameState::new(seed, map_w, map_h, *config);

                if let Ok(map_file) = sow_core::maps::load_map_from_payload(&map_bytes) {
                    state.total_land_tiles = map_file.num_land_tiles;
                    state.map_spawns = map_file.spawns;
                    if map_file.terrain.len() == state.map.terrain.len() {
                        let dest_ptr = state.map.terrain.as_mut_ptr() as *mut u8;
                        unsafe {
                            std::ptr::copy_nonoverlapping(
                                map_file.terrain.as_ptr(),
                                dest_ptr,
                                map_file.terrain.len(),
                            );
                        }
                    }
                } else if map_bytes.len() == state.map.terrain.len() {
                    let dest_ptr = state.map.terrain.as_mut_ptr() as *mut u8;
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            map_bytes.as_ptr(),
                            dest_ptr,
                            map_bytes.len(),
                        );
                    }
                }

                let water =
                    sow_core::water_components::WaterComponents::compute(&state.map, |_| {});
                let mut new_engine = sow_core::engine::SowEngine::new(state, water);

                for p in players {
                    if p.player_type == sow_core::player::PlayerType::Human {
                        new_engine.spawn_human(
                            p.id,
                            p.name,
                            p.color,
                            p.team,
                            p.civilization,
                            p.leader,
                        );
                    }
                }

                new_engine.spawn_ai(
                    new_engine.state.config.nation_count,
                    new_engine.state.config.bot_count,
                );
                let snap = new_engine.build_snapshot();
                self.sim.current_snapshot = Some(snap);
                self.sim.engine = Some(new_engine);
                self.sim.tile_upgrades = vec![0; (map_w * map_h) as usize];
                self.time
                    .interp
                    .set_tick_dur_ms(self.sim.config.tick_rate_ms);
                self.time.interp.stamp_applied(web_time::Instant::now());
                self.sim.offline_tick_timer = 0.0;
                self.sim.offline_last_update = web_time::Instant::now();
                self.ui.mover_scene = crate::render::world::movers::MoverScene::new();

                self.input.camera_zoom = 0.5;
                self.input.camera_x =
                    self.input.screen_w * 0.5 - (map_w as f32 * 0.5) * self.input.camera_zoom;
                self.input.camera_y =
                    self.input.screen_h * 0.5 - (map_h as f32 * 0.5) * self.input.camera_zoom;
                self.input.has_snapped_camera_to_spawn = false;
                self.ui.is_spectating = false;
                self.ui.endgame_cache = None;
                sow_audio::set_music_context(seed as u32, map_w as f32 * 0.5, map_h as f32 * 0.5);
            }
            sow_core::protocol::SimCommand::Turn(turn) => {
                if let Some(e) = &mut self.sim.engine {
                    e.apply_intents(&turn.intents);
                    e.tick();

                    let mut snap = e.build_snapshot();

                    // Process events produced by the engine during the tick!
                    let my_id = self.sim.my_player_id.unwrap_or(0);
                    let now_instant = web_time::Instant::now();
                    let mut turn_defeats = crate::player_progress::SessionDefeats::default();
                    let mut played_combat_this_tick = false;
                    for event in e.state.events.drain(..) {
                        if let sow_core::game::GameEvent::PlayerEliminated {
                            player_id,
                            conqueror_id,
                            gold_bounty,
                            elimination_x,
                            elimination_y,
                            assists,
                        } = event
                        {
                            let mut wx = 0.5;
                            let mut wy = 0.5;
                            let mut target_name = format!("Player {}", player_id);

                            let mut tile_found = false;
                            if elimination_x > 0 || elimination_y > 0 {
                                wx = elimination_x as f32 + 0.5;
                                wy = elimination_y as f32 + 0.5;
                                tile_found = true;
                            }

                            if let Some(target) = snap.players.iter().find(|p| p.id == player_id) {
                                target_name = sow_core::player::display_name(
                                    target.id,
                                    &target.name,
                                    target.player_type,
                                );
                                if !tile_found
                                    && (target.centroid_x > 0.001 || target.centroid_y > 0.001)
                                {
                                    wx = target.centroid_x + 0.5;
                                    wy = target.centroid_y + 0.5;
                                    tile_found = true;
                                }
                            }

                            if !tile_found {
                                // Fallback: Use conqueror's position as the visual reward point,
                                // since the conqueror just claimed the target's last tile.
                                if let Some(conqueror) =
                                    snap.players.iter().find(|p| p.id == conqueror_id)
                                {
                                    wx = conqueror.centroid_x + 0.5;
                                    wy = conqueror.centroid_y + 0.5;
                                }
                            }

                            let victim_type = snap
                                .players
                                .iter()
                                .find(|p| p.id == player_id)
                                .map(|p| p.player_type)
                                .unwrap_or(sow_core::player::PlayerType::Bot);

                            let seed = (player_id as u32)
                                .wrapping_mul(2654435761)
                                .wrapping_add(elimination_x.wrapping_mul(1597334977))
                                .wrapping_add(elimination_y.wrapping_mul(3512401961));

                            // Play retro synthesized death sound spatially
                            sow_audio::play_death_sound(
                                crate::player_sound_type(victim_type),
                                seed,
                                sow_audio::SpatialSoundParams {
                                    wx,
                                    wy,
                                    camera_x: self.input.camera_x,
                                    camera_y: self.input.camera_y,
                                    camera_zoom: self.input.camera_zoom,
                                    screen_w: self.input.screen_w,
                                    screen_h: self.input.screen_h,
                                },
                            );

                            if conqueror_id == my_id && my_id != 0 {
                                if let Some(victim) =
                                    snap.players.iter().find(|p| p.id == player_id)
                                {
                                    use sow_core::player::PlayerType;
                                    match victim.player_type {
                                        PlayerType::Human => turn_defeats.players += 1,
                                        PlayerType::Nation => turn_defeats.empires += 1,
                                        PlayerType::Bot => turn_defeats.tribes += 1,
                                    }
                                }
                            }

                            // Spawn floating notice for killer and assist contributors
                            if conqueror_id == my_id && my_id != 0 {
                                let bounty_text = format!(
                                    "🪙 +{}",
                                    sow_ui::utils::format_number(gold_bounty as f64)
                                );
                                self.ui.floating_notices.push(crate::app::FloatingNotice {
                                    text: bounty_text,
                                    world_x: wx,
                                    world_y: wy,
                                    start_time: now_instant,
                                    duration: web_time::Duration::from_millis(3000),
                                    color: egui::Color32::from_rgb(250, 204, 21),
                                });
                            }
                            for (assist_id, assist_gold) in &assists {
                                if *assist_id == my_id && my_id != 0 {
                                    let bounty_text = format!(
                                        "🪙 +{} assist",
                                        sow_ui::utils::format_number(*assist_gold as f64)
                                    );
                                    self.ui.floating_notices.push(crate::app::FloatingNotice {
                                        text: bounty_text,
                                        world_x: wx,
                                        world_y: wy + 0.5,
                                        start_time: now_instant,
                                        duration: web_time::Duration::from_millis(3000),
                                        color: egui::Color32::from_rgb(180, 220, 100),
                                    });
                                }
                            }

                            // Spawn death nameplate animations on desktop only
                            if self.input.screen_w >= 600.0 {
                                // Spawn death nameplate animation
                                let mut target_player_type = sow_core::player::PlayerType::Bot;
                                let mut player_color = egui::Color32::WHITE;
                                let mut target_nameplate_size = 0.0;

                                if let Some(target) =
                                    snap.players.iter().find(|p| p.id == player_id)
                                {
                                    target_player_type = target.player_type;
                                    player_color =
                                        crate::hud::nameplate::ensure_readable_nameplate_color(
                                            target.color,
                                        );
                                    target_nameplate_size = target.nameplate_size;
                                }

                                // Prefer smoothed label positions and sizes if available
                                let anim_wx = self
                                    .ui
                                    .label_positions
                                    .get(&player_id)
                                    .map(|p| p.0)
                                    .unwrap_or(wx);
                                let anim_wy = self
                                    .ui
                                    .label_positions
                                    .get(&player_id)
                                    .map(|p| p.1)
                                    .unwrap_or(wy);
                                let anim_size = self
                                    .ui
                                    .label_sizes
                                    .get(&player_id)
                                    .copied()
                                    .unwrap_or(target_nameplate_size)
                                    .max(0.2);

                                let seed = (player_id as u32)
                                    .wrapping_mul(2654435761)
                                    .wrapping_add(now_instant.elapsed().as_millis() as u32);
                                self.ui.death_nameplates.push(
                                    crate::app::DeathNameplateAnimation {
                                        name: target_name.clone(),
                                        color: player_color,
                                        world_x: anim_wx,
                                        world_y: anim_wy,
                                        start_time: now_instant,
                                        duration: web_time::Duration::from_millis(1200),
                                        seed,
                                        player_type: target_player_type,
                                        player_id,
                                        nameplate_size: anim_size,
                                    },
                                );
                            }

                            // Push notification message (always, including mobile!)
                            let msg = if conqueror_id == my_id && my_id != 0 {
                                format!(
                                    "🎉 You conquered {} and earned {} Gold!",
                                    target_name,
                                    sow_ui::utils::format_number(gold_bounty as f64)
                                )
                            } else if assists.iter().any(|(id, _)| *id == my_id) {
                                let assist_gold = assists
                                    .iter()
                                    .find(|(id, _)| *id == my_id)
                                    .map(|(_, g)| *g)
                                    .unwrap_or(0);
                                format!(
                                    "🤝 Assist on {} (+{} Gold)",
                                    target_name,
                                    sow_ui::utils::format_number(assist_gold as f64)
                                )
                            } else {
                                let conqueror_name = snap
                                    .players
                                    .iter()
                                    .find(|p| p.id == conqueror_id)
                                    .map(|p| p.name.clone())
                                    .unwrap_or_else(|| format!("Player {}", conqueror_id));
                                if assists.is_empty() {
                                    format!(
                                        "🕊️ {} was eliminated by {}!",
                                        target_name, conqueror_name
                                    )
                                } else {
                                    format!(
                                        "🕊️ {} was eliminated by {} (+{} assists)",
                                        target_name,
                                        conqueror_name,
                                        assists.len()
                                    )
                                }
                            };
                            self.ui
                                .app
                                .hud_state
                                .push_notification(msg, egui::Color32::from_rgb(255, 215, 0));
                        } else if let sow_core::game::GameEvent::TileCaptured {
                            x,
                            y,
                            new_owner,
                            previous_owner,
                            troops,
                        } = event
                        {
                            if played_combat_this_tick || my_id == 0 {
                                continue;
                            }
                            if new_owner != my_id && previous_owner != my_id {
                                continue;
                            }
                            played_combat_this_tick = true;

                            use sow_audio::{play_combat_sound, CombatSoundKind};
                            use sow_core::player::PlayerType;

                            let kind = if previous_owner == my_id {
                                CombatSoundKind::CounterAttack
                            } else if previous_owner == 0 {
                                CombatSoundKind::WildernessExpansion
                            } else {
                                snap.players
                                    .iter()
                                    .find(|p| p.id == previous_owner)
                                    .map(|p| match p.player_type {
                                        PlayerType::Human => CombatSoundKind::AttackHuman,
                                        PlayerType::Nation => CombatSoundKind::AttackEmpire,
                                        PlayerType::Bot => CombatSoundKind::AttackTribe,
                                    })
                                    .unwrap_or(CombatSoundKind::AttackTribe)
                            };

                            let seed = (previous_owner as u32)
                                .wrapping_mul(2654435761)
                                .wrapping_add(x.wrapping_mul(1597334977))
                                .wrapping_add(y.wrapping_mul(3512401961))
                                .wrapping_add((troops as u32).wrapping_mul(7243));

                            play_combat_sound(
                                kind,
                                troops as f32,
                                seed,
                                sow_audio::SpatialSoundParams {
                                    wx: x as f32 + 0.5,
                                    wy: y as f32 + 0.5,
                                    camera_x: self.input.camera_x,
                                    camera_y: self.input.camera_y,
                                    camera_zoom: self.input.camera_zoom,
                                    screen_w: self.input.screen_w,
                                    screen_h: self.input.screen_h,
                                },
                            );
                        } else if let sow_core::game::GameEvent::StructureSpawned {
                            tile_idx,
                            kind,
                            owner_id,
                            ..
                        } = event
                        {
                            if owner_id == my_id {
                                continue;
                            }
                            let x = (tile_idx % self.sim.map_w) as f32 + 0.5;
                            let y = (tile_idx / self.sim.map_w) as f32 + 0.5;
                            sow_audio::play_building_placement_sound(
                                crate::building_sound_kind(kind),
                                sow_audio::SpatialSoundParams {
                                    wx: x,
                                    wy: y,
                                    camera_x: self.input.camera_x,
                                    camera_y: self.input.camera_y,
                                    camera_zoom: self.input.camera_zoom,
                                    screen_w: self.input.screen_w,
                                    screen_h: self.input.screen_h,
                                },
                            );
                        }
                    }
                    self.progress_session_defeats.players = self
                        .progress_session_defeats
                        .players
                        .saturating_add(turn_defeats.players);
                    self.progress_session_defeats.empires = self
                        .progress_session_defeats
                        .empires
                        .saturating_add(turn_defeats.empires);
                    self.progress_session_defeats.tribes = self
                        .progress_session_defeats
                        .tribes
                        .saturating_add(turn_defeats.tribes);

                    if let Some(mut existing) = self.sim.current_snapshot.take() {
                        // Detect building level upgrades and completions
                        let now = web_time::Instant::now();
                        for b_new in &snap.buildings {
                            if let Some(b_old) =
                                existing.buildings.iter().find(|b| b.id == b_new.id)
                            {
                                if b_new.level > b_old.level
                                    || (b_old.under_construction && !b_new.under_construction)
                                {
                                    self.ui.active_upgrades.push(
                                        crate::app::ActiveUpgradeAnimation {
                                            tile_idx: b_new.tile_idx,
                                            start_time: now,
                                            duration: web_time::Duration::from_millis(2000),
                                            kind: b_new.kind,
                                            level: b_new.level,
                                        },
                                    );
                                }
                                if b_old.under_construction && !b_new.under_construction {
                                    let wx = (b_new.tile_idx % self.sim.map_w) as f32 + 0.5;
                                    let wy = (b_new.tile_idx / self.sim.map_w) as f32 + 0.5;
                                    sow_audio::play_building_completed_sound(
                                        crate::building_sound_kind(b_new.kind),
                                        sow_audio::SpatialSoundParams {
                                            wx,
                                            wy,
                                            camera_x: self.input.camera_x,
                                            camera_y: self.input.camera_y,
                                            camera_zoom: self.input.camera_zoom,
                                            screen_w: self.input.screen_w,
                                            screen_h: self.input.screen_h,
                                        },
                                    );
                                }
                            }
                        }

                        if !existing.dirty_tiles.is_empty() {
                            existing.dirty_tiles.append(&mut snap.dirty_tiles);
                            snap.dirty_tiles = existing.dirty_tiles;
                        }
                    }
                    // Process nuke alerts into HUD notifications
                    let my_id = self.sim.my_player_id.unwrap_or(0);
                    for alert in &snap.nuke_alerts {
                        let attacker_name = snap
                            .players
                            .iter()
                            .find(|p| p.id == alert.owner_id)
                            .map(|p| sow_core::player::display_name(p.id, &p.name, p.player_type))
                            .unwrap_or_else(|| format!("Player {}", alert.owner_id));

                        // Determine victim from tile ownership in previous snapshot state
                        let tile_idx = alert.tile_y * self.sim.map_w + alert.tile_x;
                        let victim_id = self
                            .gfx
                            .map_renderer
                            .as_ref()
                            .and_then(|mr| mr.owners.get(tile_idx as usize).copied())
                            .unwrap_or(0);
                        let victim_name = if victim_id == 0 {
                            "unclaimed territory".to_string()
                        } else {
                            snap.players
                                .iter()
                                .find(|p| p.id == victim_id)
                                .map(|p| {
                                    sow_core::player::display_name(p.id, &p.name, p.player_type)
                                })
                                .unwrap_or_else(|| format!("Player {}", victim_id))
                        };

                        let kind_str = match alert.kind {
                            sow_core::game::NukeKind::AtomBomb => "Tactical Nuke",
                        };

                        let (message, color) = if victim_id == my_id && my_id != 0 {
                            // You got nuked
                            (
                                format!(
                                    "{} launched {} on YOUR territory!",
                                    attacker_name, kind_str
                                ),
                                egui::Color32::from_rgb(239, 68, 68),
                            )
                        } else if alert.owner_id == my_id {
                            // You nuked someone
                            (
                                format!("Your {} detonated on {}", kind_str, victim_name),
                                egui::Color32::from_rgb(74, 222, 128),
                            )
                        } else if my_id != 0
                            && snap
                                .players
                                .iter()
                                .find(|p| p.id == my_id)
                                .map(|p| p.alliances.contains(&victim_id))
                                .unwrap_or(false)
                            && victim_id != 0
                        {
                            // Ally got nuked
                            (
                                format!(
                                    "{} launched {} on ally {}!",
                                    attacker_name, kind_str, victim_name
                                ),
                                egui::Color32::from_rgb(251, 191, 36),
                            )
                        } else {
                            // Enemy vs enemy / neutral
                            (
                                format!(
                                    "{} launched {} on {}",
                                    attacker_name, kind_str, victim_name
                                ),
                                egui::Color32::from_rgb(180, 180, 200),
                            )
                        };

                        self.ui.app.hud_state.push_notification(message, color);
                    }

                    let my_id = self.sim.my_player_id.unwrap_or(0);
                    let my_team = snap
                        .players
                        .iter()
                        .find(|p| p.id == my_id)
                        .and_then(|p| p.team);
                    self.maybe_submit_online_stats(&snap);
                    self.maybe_record_match_progress(snap.winner, snap.winning_team, my_team);
                    self.sim.current_snapshot = Some(snap);
                    self.time.interp.stamp_applied(web_time::Instant::now());
                }
            }
            sow_core::protocol::SimCommand::Shutdown => {
                self.sim.engine = None;
                self.sim.current_snapshot = None;
                self.ui.mover_scene = crate::render::world::movers::MoverScene::new();
            }
        }
    }
}
