use sow_render::{MapRenderer, RenderContext};

use crate::hud::nameplate::*;
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
    pub render_ctx: sow_render::RenderContext,
    pub map_renderer: Option<sow_render::MapRenderer>,
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
    pub context_menu_timer: f32,
    pub last_pinch_state: Option<(f64, f64, f64)>,
    /// Hold-to-attack: (target_owner, press_start_time, screen_x, screen_y, has_fired_initial)
    pub hold_attack_target: Option<(u16, web_time::Instant, f64, f64, bool)>,
    pub hold_attack_accum: f32,
    pub ime_allowed_state: bool,
    pub ime_cursor_rect_px: Option<egui::Rect>,
    pub has_snapped_camera_to_spawn: bool,
}

pub struct UiState {
    pub app: sow_ui::ClientApp,
    pub egui_ctx: egui::Context,
    pub raw_input: egui::RawInput,
    pub nameplate_cache: std::collections::HashMap<u16, crate::hud::nameplate::CachedNameplate>,
    pub troop_label_throttle: crate::hud::nameplate::TroopLabelThrottle,
    pub label_positions: std::collections::HashMap<u16, (f32, f32)>,
    pub tutorial_completed: bool,
    pub tutorial_step: crate::hud::tutorial::TutorialStep,
    pub show_leaderboard: bool,
    pub leaderboard_timer: f32,
    pub cached_leaderboard: Vec<(u16, String, u32, f64)>,
    pub show_dev_sidebar: bool,
    pub update_available: bool,
    pub is_spectating: bool,
}

pub struct TimeState {
    pub last_tick: web_time::Instant,
    pub start_time: web_time::Instant,
    pub tick_interval: web_time::Duration,
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
    pub(crate) ime_bridge: crate::ime::WasmImeBridge,
}

impl Default for SowApp {
    fn default() -> Self {
        Self::new()
    }
}

impl SowApp {
    pub fn new() -> Self {
        #[cfg(target_os = "android")]
        {
            android_logger::init_once(
                android_logger::Config::default()
                    .with_max_level(log::LevelFilter::Debug)
                    .with_tag("sow-client"),
            );
            std::panic::set_hook(Box::new(|info| {
                log::error!("PANIC: {}", info);
            }));
        }
        #[cfg(not(target_os = "android"))]
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
        let render_ctx = RenderContext::new();
        let surface: Option<gpu::Surface> = None;
        let map_renderer: Option<MapRenderer> = None;
        let gui_painter: Option<GuiPainter> = None;
        let window: Option<Box<dyn winit::window::Window>> = None;

        // ── UI State ────────────────────────────────────────────────────────────
        let mut app = ClientApp::new();
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
        type EngineInitData = (
            sow_core::game::GameState,
            sow_core::water_components::WaterComponents,
            sow_core::protocol::ServerStartMessage,
        );
        let (engine_init_tx, engine_init_rx) = crossbeam_channel::unbounded::<EngineInitEvent>();
        let pending_engine_init_data: Option<EngineInitData> = None;
        let engine_init_queued_msg: Option<sow_core::protocol::ServerStartMessage> = None;

        let nameplate_cache: HashMap<u16, CachedNameplate> = HashMap::new();
        let troop_label_throttle = TroopLabelThrottle::default();

        let (connect_tx, connect_rx) = crossbeam_channel::unbounded();

        // Reconnect scheduling (idle drop / resume / failed handshake).
        let ws_connect_fail_backoff_ms: u64 = 400;
        let ws_connect_not_before: Instant = Instant::now();
        let ws_reconnect_after_resume: bool = false;
        #[cfg(target_arch = "wasm32")]
        let wasm_doc_was_visible: bool = true;
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
                        let protocol = if window.location().protocol().unwrap_or_default() == "https:" { "wss" } else { "ws" };
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
        let last_tick = Instant::now();
        let start_time = Instant::now();
        let tick_interval = Duration::from_millis(50);
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
                context_menu_timer: 0.0,
                last_pinch_state,
                hold_attack_target: None,
                hold_attack_accum: 0.0,
                ime_allowed_state,
                ime_cursor_rect_px,
                has_snapped_camera_to_spawn,
            },
            ui: UiState {
                app,
                egui_ctx,
                raw_input,
                nameplate_cache,
                troop_label_throttle,
                label_positions: std::collections::HashMap::new(),
                tutorial_completed,
                tutorial_step: crate::hud::tutorial::TutorialStep::Welcome,
                show_leaderboard: false,
                leaderboard_timer: 0.0,
                cached_leaderboard: Vec::new(),
                show_dev_sidebar: false,
                update_available: false,
                is_spectating: false,
            },
            time: TimeState {
                last_tick,
                start_time,
                tick_interval,
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
            ime_bridge,
        }
    }

    /// Tear down an online match and run the existing ExitGame splash → MainMenu flow.
    pub(crate) fn begin_exit_to_main_menu(&mut self) {
        self.net.is_offline = false;
        self.net.ws_url = self.net.orchestrator_url.clone();
        self.ui.app.main_menu_state.server_address = self.net.ws_url.clone();

        // Drop relay connection and force orchestrator reconnect
        self.net.client = None;
        self.ui.app.main_menu_state.is_connected = false;
        self.ui.app.main_menu_state.is_connecting = false;
        while let Ok(_) = self.net.connect_rx.try_recv() {}
        self.net.ws_connect_not_before = web_time::Instant::now();

        self.ui.app.main_menu_state.is_waiting = false;
        self.ui.app.main_menu_state.pending_join_lobby_id = None;
        self.ui.app.main_menu_state.joined_lobby_id = None;
        self.ui.app.hud_state.sync_state = None;
        self.sim.my_lobby_id = None;
        self.sim.my_player_id = None;
        self.ui.app.phase = ClientPhase::Splash;
        let lang = self.ui.app.settings_state.language;
        self.ui.app.splash_state.reset_anim(sow_ui::ui::loading_screen::SplashJob::ExitGame, lang);
        self.ui.app.splash_state.select_voluntary_exit(lang);
        self.ui.is_spectating = false;
    }

    #[inline]
    pub(crate) fn ws_on_relay(&self) -> bool {
        self.net.ws_url.contains("/relay/") || self.net.ws_url.contains("2557")
    }

    pub fn handle_suspended(&mut self, _event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        if let Some(sp) = self.gfx.prev_sync_point.take() {
            let _ = self.gfx.render_ctx.context.wait_for(&sp, !0);
        }
        if let Some(mut s) = self.gfx.surface.take() {
            if let Some(mut gp) = self.gfx.gui_painter.take() {
                gp.destroy(&self.gfx.render_ctx.context);
            }
            if let Some(mut mr) = self.gfx.map_renderer.take() {
                mr.destroy(&self.gfx.render_ctx);
            }
            self.gfx.render_ctx.context.destroy_surface(&mut s);
        }
    }

    pub fn handle_resumed(&mut self, event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        // App or tab foregrounded — retry WS soon if the socket died in the background.
        self.net.ws_reconnect_after_resume = true;
        if self.gfx.window.is_none() {
            #[cfg(any(target_os = "android", target_os = "ios"))]
            let attributes =
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
                let web_attrs = winit::platform::web::WindowAttributesWeb::default()
                    .with_canvas(Some(canvas))
                    .with_prevent_default(false);
                attributes = attributes.with_platform_attributes(Box::new(web_attrs));
                crate::ime::ensure_canvas_tabindex();
            }

            #[cfg(not(any(target_os = "android", target_os = "ios", target_family = "wasm")))]
            let attributes = {
                let attrs = winit::window::WindowAttributes::default()
                    .with_title("Shadows of War — Native")
                    .with_surface_size(winit::dpi::LogicalSize::new(800.0, 800.0));
                attrs
            };

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
        if let Some(sp) = self.gfx.prev_sync_point.take() {
            let _ = self.gfx.render_ctx.context.wait_for(&sp, !0);
        }
        if let Some(mut mr) = self.gfx.map_renderer.take() {
            mr.destroy(&self.gfx.render_ctx);
        }
        if let Some(mut gui) = self.gfx.gui_painter.take() {
            gui.destroy(&self.gfx.render_ctx.context);
        }
    }
}

impl SowApp {
    pub fn update(&mut self) {
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
                nations,
            } => {
                let map_w = config.map_width;
                let map_h = config.map_height;
                let mut state = sow_core::game::GameState::new(seed, map_w, map_h, *config);

                if map_bytes.len() == state.map.terrain.len() {
                    let dest_ptr = state.map.terrain.as_mut_ptr() as *mut u8;
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            map_bytes.as_ptr(),
                            dest_ptr,
                            map_bytes.len(),
                        );
                    }
                } else {
                    for (i, &b) in map_bytes.iter().enumerate() {
                        if i < state.map.terrain.len() {
                            state.map.terrain[i] = sow_core::map::MapTile::from_byte(b);
                        }
                    }
                }

                let water =
                    sow_core::water_components::WaterComponents::compute(&state.map, |_| {});
                let mut new_engine = sow_core::engine::SowEngine::new(state, water);

                for p in players {
                    if p.player_type == sow_core::player::PlayerType::Human {
                        new_engine.spawn_human(p.id, p.name, p.color, p.team);
                    }
                }

                new_engine.spawn_ai(
                    new_engine.state.config.nation_count,
                    new_engine.state.config.bot_count,
                    nations,
                );
                let snap = new_engine.build_snapshot();
                self.sim.current_snapshot = Some(snap);
                self.sim.engine = Some(new_engine);

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
                    if let Some(mut existing) = self.sim.current_snapshot.take() {
                        if !existing.dirty_tiles.is_empty() {
                            existing.dirty_tiles.append(&mut snap.dirty_tiles);
                            snap.dirty_tiles = existing.dirty_tiles;
                        }
                    }
                    self.sim.current_snapshot = Some(snap);
                }
            }
            sow_core::protocol::SimCommand::Shutdown => {
                self.sim.engine = None;
                self.sim.current_snapshot = None;
            }
        }
    }
}
