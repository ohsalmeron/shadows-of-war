use sow_render::{MapRenderer, RenderContext};

use crate::spawn_sow_client_connect;
use crate::{EngineInitEvent, MapDownloadEvent};
use blade_egui::GuiPainter;
use blade_graphics as gpu;
use egui::{Context, RawInput, Rect};
use sow_core::protocol::SimSnapshot;
use sow_net::client::SowClient;
use sow_ui::{app::ClientPhase, ClientApp};
use std::collections::HashMap;
use web_time::{Duration, Instant};

pub struct GraphicsState {
    pub window: Option<Box<dyn winit::window::Window>>,
    pub surface: Option<blade_graphics::Surface>,
    pub render_ctx: Option<sow_render::RenderContext>,
    pub map_renderer: Option<sow_render::MapRenderer>,
    pub mover_renderer: Option<sow_render::MoverRenderer>,
    pub gui_painter: Option<blade_egui::GuiPainter>,
    pub prev_sync_point: Option<blade_graphics::SyncPoint>,
    pub needs_first_upload: bool,
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

#[derive(Clone, Copy, Debug)]
pub enum ExplosionKind {
    Atom,
    Hydrogen,
    MIRVWarhead,
}

#[derive(Clone, Debug)]
pub struct ActiveExplosion {
    pub x: f32,
    pub y: f32,
    pub start_time: web_time::Instant,
    pub max_radius: f32,
    pub kind: ExplosionKind,
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
}

#[allow(clippy::type_complexity)]
pub struct UiState {
    pub app: sow_ui::ClientApp,
    pub egui_ctx: egui::Context,
    pub raw_input: egui::RawInput,

    pub label_positions: std::collections::HashMap<u16, (f32, f32)>,
    pub tutorial_completed: bool,
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
    pub active_explosions: Vec<ActiveExplosion>,
    pub fallout_zones: Vec<FalloutZone>,
    pub last_projectiles: std::collections::HashMap<u64, sow_core::protocol::ProjectileSnapshot>,
    pub active_upgrades: Vec<ActiveUpgradeAnimation>,
    pub nameplate_galleys: std::collections::HashMap<
        u16,
        (
            String,
            String,
            egui::FontId,
            std::sync::Arc<egui::Galley>,
            std::sync::Arc<egui::Galley>,
        ),
    >,
    pub cached_player_colors: Vec<egui::Color32>,
    pub cached_player_count: usize,
    pub star_svg_registered: bool,
    pub handshake_svg_registered: bool,
    pub floating_notices: Vec<FloatingNotice>,
    pub death_nameplates: Vec<DeathNameplateAnimation>,

    pub cached_hovered_building_id: Option<u64>,
    pub cached_hovered_building_level: u8,
    pub cached_hovered_building_tooltip: String,
    pub attack_troop_labels: std::collections::HashMap<u64, (f64, String)>,
    pub edge_mask_cache: Vec<u8>,
    pub rail_state: crate::render::world::railways::RailState,
    /// Client-side nuke silo cooldown tracking: building id → tick when ready.
    pub silo_cooldowns: std::collections::HashMap<u64, u64>,
    pub mover_scene: crate::render::world::movers::MoverScene,
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
    #[cfg(target_arch = "wasm32")]
    pub(crate) web_loader_hidden: bool,
    #[cfg(target_arch = "wasm32")]
    pub(crate) ime_bridge: crate::ime::WasmImeBridge,
    pub map_editor: Option<sow_map::MapEditorSession>,
    /// Set when Blade/Vulkan init fails; event loop exits on next tick.
    pub gpu_init_failed: bool,
}

impl Default for SowApp {
    fn default() -> Self {
        Self::new()
    }
}

impl SowApp {
    pub fn new() -> Self {
        #[cfg(all(not(target_arch = "wasm32")))]
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
        let mut app = ClientApp::new();
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
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Ok(bytes) = std::fs::read("assets/maps/catalog.bin") {
                if let Ok(catalog) = sow_core::map_file::parse_catalog(&bytes) {
                    app.main_menu_state.apply_map_catalog(&catalog.entries);
                    app.asset_loader.map_catalog = Some(catalog.entries);
                }
            }
        }
        let egui_ctx = Context::default();
        egui_extras::install_image_loaders(&egui_ctx);
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

        let mut tutorial_completed = false;
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(val)) = window.local_storage().and_then(|s| {
                    Ok(s.and_then(|st| st.get_item("sow_tutorial_completed").ok().flatten()))
                }) {
                    if val == "true" {
                        tutorial_completed = true;
                    }
                }
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if std::path::Path::new("sow_tutorial_completed.txt").exists() {
                tutorial_completed = true;
            }
        }

        Self {
            gfx: GraphicsState {
                window,
                surface,
                render_ctx,
                map_renderer,
                mover_renderer,
                gui_painter,
                prev_sync_point,
                needs_first_upload,
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
                tutorial_completed,
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
                active_explosions: Vec::new(),
                fallout_zones: Vec::new(),
                last_projectiles: std::collections::HashMap::new(),
                active_upgrades: Vec::new(),
                nameplate_galleys: std::collections::HashMap::new(),
                cached_player_colors: Vec::new(),
                cached_player_count: 0,
                star_svg_registered: false,
                handshake_svg_registered: false,
                floating_notices: Vec::new(),
                death_nameplates: Vec::new(),

                cached_hovered_building_id: None,
                cached_hovered_building_level: 0,
                cached_hovered_building_tooltip: String::new(),
                attack_troop_labels: std::collections::HashMap::new(),
                edge_mask_cache: Vec::new(),
                rail_state: crate::render::world::railways::RailState::new(),
                silo_cooldowns: std::collections::HashMap::new(),
                mover_scene: crate::render::world::movers::MoverScene::new(),
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
                pending_engine_init_data,
                engine_init_queued_msg,
            },
            #[cfg(not(target_arch = "wasm32"))]
            tokio_rt,
            #[cfg(target_arch = "wasm32")]
            wasm_doc_was_visible,
            #[cfg(target_arch = "wasm32")]
            web_loader_hidden,
            #[cfg(target_arch = "wasm32")]
            ime_bridge,
            map_editor: None,
            gpu_init_failed: false,
        }
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

    /// Tear down an online match and run the existing ExitGame splash → MainMenu flow.
    pub(crate) fn begin_exit_to_main_menu(&mut self, use_loader: bool) {
        if self.ui.app.phase == sow_ui::app::ClientPhase::Playing {
            crate::store_portals::gameplay_stop();
        }
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
        }
        self.ui.is_spectating = false;
    }

    #[inline]
    pub(crate) fn ws_on_relay(&self) -> bool {
        self.net.ws_url.contains("/relay/") || self.net.ws_url.contains("2557")
    }

    /// Window used for input/redraw: map editor session owns it while editing.
    pub fn active_window(&self) -> Option<&dyn winit::window::Window> {
        if let Some(editor) = self.map_editor.as_ref() {
            return editor.window_ref();
        }
        self.gfx.window.as_deref()
    }

    pub fn handle_suspended(&mut self, _event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        if let Some(ref mut editor) = self.map_editor {
            editor.handle_suspended();
            return;
        }
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
        if let Some(ref mut editor) = self.map_editor {
            editor.handle_resumed();
            return;
        }
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
                    .with_prefers_home_indicator_hidden(true);
                attributes = attributes.with_platform_attributes(Box::new(ios_attrs));
            }
            #[cfg(target_arch = "wasm32")]
            let mut attributes = {
                let window = web_sys::window().unwrap();
                let w = window.inner_width().unwrap().as_f64().unwrap();
                let h = window.inner_height().unwrap().as_f64().unwrap();
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
                let is_mobile = window
                    .navigator()
                    .user_agent()
                    .map(|ua| {
                        let ua = ua.to_lowercase();
                        ua.contains("mobi")
                            || ua.contains("android")
                            || ua.contains("iphone")
                            || ua.contains("ipad")
                            || ua.contains("touch")
                    })
                    .unwrap_or(false);
                let web_attrs = winit::platform::web::WindowAttributesWeb::default()
                    .with_canvas(Some(canvas))
                    .with_prevent_default(is_mobile);
                attributes = attributes.with_platform_attributes(Box::new(web_attrs));
                crate::ime::ensure_canvas_tabindex();
            }

            #[cfg(not(any(target_os = "android", target_os = "ios", target_family = "wasm")))]
            let attributes = winit::window::WindowAttributes::default()
                .with_title("Shadows of War — Native")
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
    pub fn update(&mut self, event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        if self.map_editor.is_some() {
            if let Some(mut session) = self.map_editor.take() {
                let action = session.update(event_loop);
                self.map_editor = Some(session);
                if let Some(sow_ui::UiAction::LeaveLobby) = action {
                    self.teardown_map_editor_and_exit();
                }
            }
            return;
        }

        self.check_surface();

        let now = web_time::Instant::now();
        self.update_net(now);
        self.update_assets();
        self.update_loader();
        self.update_sim(now);
    }

    pub(crate) fn teardown_map_editor_and_exit(&mut self) {
        if let Some(session) = self.map_editor.take() {
            let (window, surface, render_ctx, gui_painter, client_app, egui_ctx) =
                session.destroy_and_reclaim();
            self.gfx.window = window;
            self.gfx.surface = surface;
            self.gfx.render_ctx = Some(render_ctx);
            self.ui.app = client_app;
            self.ui.app.phase = ClientPhase::MainMenu;
            self.ui.egui_ctx = egui_ctx;
            self.gfx.prev_sync_point = None;
            self.gfx.needs_first_upload = true;
            self.gfx.gui_painter = gui_painter;
            self.reset_ui_after_editor();

            log::info!("Reclaimed graphics state from map editor session.");
            let _ = sow_map::MapEditorSession::reload_local_map_catalog(
                &mut self.ui.app,
                &self.ui.egui_ctx,
                None,
            );
            self.check_surface();
        }
    }

    /// Re-sync egui input/layout with the reclaimed window after map editor exit.
    fn reset_ui_after_editor(&mut self) {
        self.ui.raw_input.events.clear();
        if let Some(win) = self.gfx.window.as_ref() {
            let sz = win.surface_size();
            self.input.screen_w = sz.width as f32;
            self.input.screen_h = sz.height as f32;
            let sf = win.scale_factor() as f32;
            self.ui.egui_ctx.set_pixels_per_point(sf);
            self.ui.raw_input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(self.input.screen_w / sf, self.input.screen_h / sf),
            ));
        }
    }

    pub fn dispatch_sim_command(&mut self, cmd: sow_core::protocol::SimCommand) {
        match cmd {
            sow_core::protocol::SimCommand::Init {
                config,
                seed,
                map_bytes,
                players,
            } => {
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
                self.time.interp.set_tick_dur_ms(self.sim.config.tick_rate_ms);
                self.time.interp.stamp_applied(web_time::Instant::now());
                self.ui.mover_scene = crate::render::world::movers::MoverScene::new();

                self.input.camera_zoom = 0.5;
                self.input.camera_x =
                    self.input.screen_w * 0.5 - (map_w as f32 * 0.5) * self.input.camera_zoom;
                self.input.camera_y =
                    self.input.screen_h * 0.5 - (map_h as f32 * 0.5) * self.input.camera_zoom;
                self.input.has_snapped_camera_to_spawn = false;
                self.ui.is_spectating = false;
            }
            sow_core::protocol::SimCommand::Turn(turn) => {
                if let Some(e) = &mut self.sim.engine {
                    e.apply_intents(&turn.intents);
                    e.tick();

                    let mut snap = e.build_snapshot();

                    // Process events produced by the engine during the tick!
                    let my_id = self.sim.my_player_id.unwrap_or(0);
                    let now_instant = web_time::Instant::now();
                    for event in e.state.events.drain(..) {
                        if let sow_core::game::GameEvent::PlayerEliminated {
                            player_id,
                            conqueror_id,
                            gold_bounty,
                            elimination_x,
                            elimination_y,
                        } = event
                        {
                            let mut wx = 0.5;
                            let mut wy = 0.5;
                            let mut target_name = format!("Player {}", player_id);

                            let mut tile_found = false;
                            if elimination_x > 0 || elimination_y > 0 {
                                wx = elimination_x as f32 + 0.5 + (elimination_y % 2) as f32 * 0.5;
                                wy = (elimination_y as f32 + 0.5) * 0.8660254_f32;
                                tile_found = true;
                            }

                            if let Some(target) = snap.players.iter().find(|p| p.id == player_id) {
                                target_name = if target.name.is_empty() {
                                    if target.id >= 200 {
                                        format!("Tribe {}", target.id - 199)
                                    } else {
                                        format!("Nation {}", target.id.saturating_sub(103))
                                    }
                                } else {
                                    target.name.clone()
                                };
                                if !tile_found
                                    && (target.centroid_x > 0.001 || target.centroid_y > 0.001)
                                {
                                    wx = target.centroid_x
                                        + 0.5
                                        + (target.centroid_y as i32 % 2) as f32 * 0.5;
                                    wy = (target.centroid_y + 0.5) * 0.8660254_f32;
                                    tile_found = true;
                                }
                            }

                            if !tile_found {
                                // Fallback: Use conqueror's position as the visual reward point,
                                // since the conqueror just claimed the target's last tile.
                                if let Some(conqueror) =
                                    snap.players.iter().find(|p| p.id == conqueror_id)
                                {
                                    wx = conqueror.centroid_x
                                        + 0.5
                                        + (conqueror.centroid_y as i32 % 2) as f32 * 0.5;
                                    wy = (conqueror.centroid_y + 0.5) * 0.8660254_f32;
                                }
                            }

                            // Spawn floating notice!
                            let bounty_text =
                                format!("💰 +{}", sow_ui::utils::format_number(gold_bounty as f64));
                            self.ui.floating_notices.push(crate::app::FloatingNotice {
                                text: bounty_text,
                                world_x: wx,
                                world_y: wy,
                                start_time: now_instant,
                                duration: web_time::Duration::from_millis(3000),
                                color: egui::Color32::from_rgb(250, 204, 21),
                            });

                            // Spawn death nameplate animations on desktop only
                            if self.input.screen_w >= 600.0 {
                                // Spawn death nameplate animation
                                let player_color = snap
                                    .players
                                    .iter()
                                    .find(|p| p.id == player_id)
                                    .map(|p| {
                                        egui::Color32::from_rgb(
                                            (p.color[0] * 255.0) as u8,
                                            (p.color[1] * 255.0) as u8,
                                            (p.color[2] * 255.0) as u8,
                                        )
                                    })
                                    .unwrap_or(egui::Color32::WHITE);
                                let seed = (player_id as u32)
                                    .wrapping_mul(2654435761)
                                    .wrapping_add(now_instant.elapsed().as_millis() as u32);
                                self.ui.death_nameplates.push(
                                    crate::app::DeathNameplateAnimation {
                                        name: format!("🕊️ {}", target_name),
                                        color: player_color,
                                        world_x: wx,
                                        world_y: wy,
                                        start_time: now_instant,
                                        duration: web_time::Duration::from_millis(3500),
                                        seed,
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
                            } else {
                                let conqueror_name = snap
                                    .players
                                    .iter()
                                    .find(|p| p.id == conqueror_id)
                                    .map(|p| p.name.clone())
                                    .unwrap_or_else(|| format!("Player {}", conqueror_id));
                                format!("🕊️ {} was eliminated by {}!", target_name, conqueror_name)
                            };
                            self.ui
                                .app
                                .hud_state
                                .push_notification(msg, egui::Color32::from_rgb(255, 215, 0));
                        }
                    }

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
                            .map(|p| {
                                if p.name.is_empty() {
                                    if p.id >= 200 {
                                        format!("Tribe {}", p.id - 199)
                                    } else {
                                        format!("Nation {}", p.id.saturating_sub(103))
                                    }
                                } else {
                                    p.name.clone()
                                }
                            })
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
                                    if p.name.is_empty() {
                                        if p.id >= 200 {
                                            format!("Tribe {}", p.id - 199)
                                        } else {
                                            format!("Nation {}", p.id.saturating_sub(103))
                                        }
                                    } else {
                                        p.name.clone()
                                    }
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
