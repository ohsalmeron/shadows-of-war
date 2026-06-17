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

/// Physical viewport size from the browser window (`innerWidth/Height × devicePixelRatio`).
#[cfg(target_arch = "wasm32")]
pub fn physical_viewport_size() -> (u32, u32) {
    let (w, h) = canvas_logical_size();
    let dpr = web_sys::window()
        .map(|window| window.device_pixel_ratio())
        .unwrap_or(1.0);
    ((w * dpr).round() as u32, (h * dpr).round() as u32)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn physical_viewport_size() -> (u32, u32) {
    (800, 600)
}

/// Set `#blade` backing-store pixels before Blade reconfigures the WebGL surface.
#[cfg(target_arch = "wasm32")]
pub fn set_canvas_backing_store_size(width: u32, height: u32) {
    use wasm_bindgen::JsCast;
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(canvas) = document
        .get_element_by_id("blade")
        .and_then(|el| el.dyn_into::<web_sys::HtmlCanvasElement>().ok())
    else {
        return;
    };
    canvas.set_width(width);
    canvas.set_height(height);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn set_canvas_backing_store_size(_width: u32, _height: u32) {}
