#![warn(dead_code, unused_variables, unused_imports)]
use sow_net::client::SowClient;

fn get_build_version() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(val) = js_sys::Reflect::get(
                &window,
                &wasm_bindgen::JsValue::from_str("SOW_BUILD_VERSION"),
            ) {
                if let Some(s) = val.as_string() {
                    if s != "__BUILD_TS__" {
                        return s;
                    }
                }
            }
        }
        "unknown".to_string()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        include_str!("../../.version").trim().to_string()
    }
}

mod asset_config;
mod config;

pub use asset_config::AssetConfig;

/// Allow very wide map views (scroll / pinch clamp to this minimum).
const CAMERA_MIN_ZOOM: f32 = 0.75;
/// Hard ceiling so zoom stays finite and GPU paths stay well-behaved.
const CAMERA_MAX_ZOOM_CAP: f32 = 100.0;

/// Pixels-per-world-unit zoom max scales with window size so you can fill ~one hex tile
/// across the long screen axis (hex neighbor spacing ≈ 1 world unit in the map shader).
fn camera_zoom_upper_bound(screen_w: f32, screen_h: f32) -> f32 {
    let longest = screen_w.max(screen_h).max(1.0);
    (longest * 3.0).clamp(CAMERA_MIN_ZOOM, CAMERA_MAX_ZOOM_CAP.max(CAMERA_MIN_ZOOM))
}

fn spawn_sow_client_connect(
    url: String,
    connect_tx: &crossbeam_channel::Sender<Result<SowClient, String>>,
    #[cfg(not(target_arch = "wasm32"))] tokio_rt: &tokio::runtime::Runtime,
) {
    let tx = connect_tx.clone();
    let url_clone = url.clone();

    #[cfg(not(target_arch = "wasm32"))]
    {
        let fut = async move {
            match tokio::time::timeout(std::time::Duration::from_secs(5), SowClient::connect(&url_clone)).await {
                Ok(Ok(c)) => {
                    let _ = tx.send(Ok(c));
                }
                Ok(Err(e)) => {
                    let _ = tx.send(Err(e.to_string()));
                }
                Err(_) => {
                    let _ = tx.send(Err("Connection timed out after 5 seconds".to_string()));
                }
            }
        };
        tokio_rt.spawn(fut);
    }

    #[cfg(target_arch = "wasm32")]
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use wasm_bindgen::JsCast;

        let tx_for_timeout = connect_tx.clone();
        let finished = Arc::new(AtomicBool::new(false));
        let finished_clone = finished.clone();

        // Spawn the connection attempt
        wasm_bindgen_futures::spawn_local(async move {
            let res = SowClient::connect(&url_clone).await;
            if !finished.swap(true, Ordering::SeqCst) {
                match res {
                    Ok(c) => {
                        let _ = tx.send(Ok(c));
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string()));
                    }
                }
            }
        });

        // Set up the timeout
        let closure = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
            if !finished_clone.swap(true, Ordering::SeqCst) {
                let _ = tx_for_timeout.send(Err("Connection timed out after 5 seconds".to_string()));
            }
        });

        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                5000,
            );
            closure.forget(); // Keep the closure alive so JS can invoke it
        }
    }
}

pub enum MapDownloadEvent {
    CatalogReady(Vec<sow_core::maps::MapCatalogEntry>),
    MapReady(String, Vec<u8>),
    ThumbnailReady(String, Vec<u8>),
    ThumbnailFailed(String, String),
    LeaderPortraitReady {
        leader: sow_core::player::Leader,
        mobile: bool,
        bytes: Vec<u8>,
    },
    LeaderPortraitFailed {
        leader: sow_core::player::Leader,
        mobile: bool,
        reason: String,
    },
    BootUiReady {
        kind: sow_ui::ui::asset_loader::UiSplashTexture,
        bytes: Vec<u8>,
    },
    BootUiFailed {
        kind: sow_ui::ui::asset_loader::UiSplashTexture,
        reason: String,
    },
    /// `leader == None` is the null/fallback avatar (`null.webp`).
    AvatarReady {
        leader: Option<sow_core::player::Leader>,
        bytes: Vec<u8>,
    },
    AvatarFailed {
        leader: Option<sow_core::player::Leader>,
        reason: String,
    },
    Progress(String, u8),
    Error(String),
}

pub enum EngineInitEvent {
    Status(String),
    Progress(f32),
    Complete(
        Box<sow_core::game::GameState>,
        sow_core::water_components::WaterComponents,
        Box<sow_core::protocol::ServerStartMessage>,
    ),
}

pub mod app;
pub mod asset;
pub mod audio;
pub mod hud;
#[cfg(target_arch = "wasm32")]
mod ime;
pub mod input;
pub mod loader;
mod map_cache;
#[cfg(target_arch = "wasm32")]
mod map_download;
pub mod net;
#[cfg(not(target_arch = "wasm32"))]
mod paths;
pub mod platform_identity;
mod guest_id;
pub mod player_progress;
mod platform_output;
pub mod render;
pub mod store_portals;
mod viewport;
mod web_canvas;

use app::SowApp;
use winit::application::ApplicationHandler;

impl ApplicationHandler for SowApp {
    fn resumed(&mut self, event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        // App resumed, might need to re-init some things on iOS
        self.handle_resumed(event_loop);
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        self.handle_resumed(event_loop);
    }

    fn suspended(&mut self, event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        self.handle_suspended(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &dyn winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        if self.gfx.window.is_none() || self.gfx.window.as_ref().unwrap().id() != window_id {
            return;
        }
        if let winit::event::WindowEvent::RedrawRequested = event {
            self.render_frame(event_loop);
        } else {
            self.handle_window_event(event_loop, event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        if self.gpu_init_failed {
            event_loop.exit();
            return;
        }
        self.update(event_loop);

        // Dynamically adjust winit control flow: Poll during active gameplay to ensure simulation
        // ticks and nameplate/ui transitions animate smoothly even when there are no user input events,
        // and Wait in other screens (like Main Menu) to preserve CPU and battery.
        #[cfg(not(target_arch = "wasm32"))]
        let flow = if self.ui.app.phase == sow_ui::app::ClientPhase::Playing {
            winit::event_loop::ControlFlow::Poll
        } else {
            winit::event_loop::ControlFlow::Wait
        };
        #[cfg(target_arch = "wasm32")]
        let flow = winit::event_loop::ControlFlow::Wait;

        event_loop.set_control_flow(flow);

        if let Some(win) = self.active_window() {
            win.request_redraw();
        }
    }
}

pub fn run_game(event_loop: winit::event_loop::EventLoop) {
    #[cfg(target_arch = "wasm32")]
    map_download::install_wasm_map_export_hook();
    let app = SowApp::new();
    let _ = event_loop.run_app(app);
}

#[cfg(target_os = "android")]
#[no_mangle]
pub fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use winit::event_loop::EventLoopBuilder;
    use winit::platform::android::EventLoopBuilderExtAndroid;

    // Redirect all crashes and logs to a physical file so we can see what's failing without ADB!
    if let Some(ext_path) = app.external_data_path() {
        let _ = std::fs::create_dir_all(&ext_path);
        let log_file = ext_path.join("sow_crash.txt");
        if let Ok(file) = std::fs::File::create(&log_file) {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            unsafe {
                libc::dup2(fd, libc::STDERR_FILENO);
                libc::dup2(fd, libc::STDOUT_FILENO);
            }
        }
    }

    // Now env_logger will write to the redirected stderr instead of logcat!
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .try_init();

    log::info!("SOW ENGINE STARTING...");

    let event_loop = EventLoopBuilder::default()
        .with_android_app(app)
        .build()
        .unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    run_game(event_loop);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn wasm_main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Info).expect("error initializing logger");
    log::info!("SOW ENGINE WASM STARTING...");

    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    run_game(event_loop);
}
pub mod sim;
