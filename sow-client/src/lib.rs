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
    let fut = async move {
        match SowClient::connect(&url).await {
            Ok(c) => {
                let _ = tx.send(Ok(c));
            }
            Err(e) => {
                let _ = tx.send(Err(e.to_string()));
            }
        }
    };
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(fut);
    #[cfg(not(target_arch = "wasm32"))]
    tokio_rt.spawn(fut);
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
pub mod hud;
#[cfg(target_arch = "wasm32")]
mod ime;
pub mod input;
pub mod loader;
pub mod net;
pub mod render;
pub mod store_portals;
#[cfg(not(target_arch = "wasm32"))]
mod paths;
mod map_cache;
mod platform_output;
#[cfg(target_arch = "wasm32")]
mod map_download;
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
