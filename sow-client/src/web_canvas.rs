//! Web viewport logical size for wasm32.
//!
//! Fullscreen shells (play, CrazyGames) use `window.innerWidth` / `innerHeight`.
//! `winit` emits `SurfaceResized` after `request_surface_size`; [`crate::input`]
//! reconfigures the GPU surface and updates egui from that event.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

/// Logical viewport size from the browser window (not `#blade` client box).
#[cfg(target_arch = "wasm32")]
pub fn canvas_logical_size() -> (f64, f64) {
    let Some(window) = web_sys::window() else {
        return (800.0, 600.0);
    };
    let w = window
        .inner_width()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(800.0);
    let h = window
        .inner_height()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(600.0);
    (w, h)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn canvas_logical_size() -> (f64, f64) {
    (800.0, 600.0)
}
