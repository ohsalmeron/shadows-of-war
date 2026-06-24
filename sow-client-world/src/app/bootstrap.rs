use super::state::*;
use crate::{spawn_sow_client_connect, EngineInitEvent, MapDownloadEvent};
use blade_egui::GuiPainter;
use blade_graphics as gpu;
use egui::{Context, RawInput, Rect};
use sow_core::protocol::SimSnapshot;
use sow_net::client::SowClient;
use sow_render::{MapRenderer, RenderContext};
use sow_ui::ClientApp;
use std::collections::HashMap;
use web_time::{Duration, Instant};

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
        sow_ui_kit::theme::apply_theme(&egui_ctx);
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
                paused: false,
            },
            input: InputState {
                camera_x,
                camera_y,
                camera_zoom,
                target_zoom: camera_zoom,
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
                key_pan_up: false,
                key_pan_down: false,
                key_pan_left: false,
                key_pan_right: false,
            },
            ui: UiState {
                app,
                egui_ctx,
                raw_input,

                label_positions: std::collections::HashMap::new(),
                label_sizes: std::collections::HashMap::new(),
                tutorial_active: false,
                tutorial_step: crate::hud::tutorial::TutorialStep::Welcome,
                tutorial_step_idx: 0,
                tutorial_baseline_tiles: 0,
                tutorial_baseline_set: false,
                tutorial_objectives_open: true,
                tutorial_modal_dismissed: false,
                tutorial_obj_done_at: std::collections::HashMap::new(),
                tutorial_last_kills: 0,
                tutorial_met_tribes: std::collections::HashSet::new(),
                tutorial_pending_intro: None,
                show_leaderboard: false,
                leaderboard_timer: 0.0,
                leaderboard_rankings: Vec::new(),
                leaderboard_display: std::collections::HashMap::new(),
                leaderboard_visible_limit: sow_ui_game::INITIAL_VISIBLE_LIMIT,
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
}
