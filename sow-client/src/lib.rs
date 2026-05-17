#![warn(dead_code, unused_variables, unused_imports)]
use sow_net::client::SowClient;



fn get_build_version() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(val) = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("SOW_BUILD_VERSION")) {
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

fn get_maps_url() -> String {
    #[allow(unused_mut)]
    let mut url = std::env::var("SOW_MAPS_URL").unwrap_or_else(|_| "https://shadowsofwar.io/assets/maps".to_string());
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(val) = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("SOW_MAPS_URL")) {
                if let Some(s) = val.as_string() {
                    url = s;
                }
            }
        }
    }
    url
}




mod config;

/// Allow very wide map views (scroll / pinch clamp to this minimum).
const CAMERA_MIN_ZOOM: f32 = 0.75;
/// Hard ceiling so zoom stays finite and GPU paths stay well-behaved.
const CAMERA_MAX_ZOOM_CAP: f32 = 50.0;

/// Pixels-per-world-unit zoom max scales with window size so you can fill ~one hex tile
/// across the long screen axis (hex neighbor spacing ≈ 1 world unit in the map shader).
fn camera_zoom_upper_bound(screen_w: f32, screen_h: f32) -> f32 {
    let longest = screen_w.max(screen_h).max(1.0);
    (longest * 3.0).clamp(CAMERA_MIN_ZOOM, CAMERA_MAX_ZOOM_CAP.max(CAMERA_MIN_ZOOM))
}

/// Zoom level used only for nameplate **font** sizing (not LOD). Matches default `camera_zoom` so
/// first-frame text size matches the old formula, while zooming no longer churns egui glyph atlas sizes.
const NAMEPLATE_REFERENCE_ZOOM: f32 = 2.0;



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
    MapReady(String, Vec<u8>),
    ThumbnailReady(String, Vec<u8>),
    Progress(String, u8),
    Error(String),
}

pub enum EngineInitEvent {
    Status(String),
    Progress(f32),
    Complete(Box<sow_core::game::GameState>, sow_core::water_components::WaterComponents, Box<sow_core::protocol::ServerStartMessage>),
}


pub mod app;
pub mod input;
pub mod render;
pub mod hud;
pub mod net;
pub mod asset;
pub mod loader;
#[cfg(target_arch = "wasm32")]
mod ime;

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

    fn about_to_wait(&mut self, _event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        self.update();
        if let Some(win) = self.gfx.window.as_ref() {
            win.request_redraw();
        }
    }
}

pub fn run_game(event_loop: winit::event_loop::EventLoop) {
    let app = SowApp::new();
    let _ = event_loop.run_app(app);
}

#[cfg(target_os = "android")]
#[no_mangle]
pub fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;
    use winit::event_loop::EventLoopBuilder;

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
    env_logger::builder().filter_level(log::LevelFilter::Info).init();

    log::info!("SOW ENGINE STARTING...");

    let event_loop = EventLoopBuilder::default().with_android_app(app).build().unwrap();
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
