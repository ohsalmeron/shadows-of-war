//! Web viewport logical size for wasm32.
//!
//! Uses `visualViewport` when available so egui layout matches the visible canvas
//! (mobile browser chrome, dynamic toolbars). `winit` emits `SurfaceResized` after
//! `request_surface_size`; [`crate::input`] reconfigures the GPU surface from that event.


#[cfg(target_arch = "wasm32")]
fn parse_css_px(s: &str) -> f32 {
    let s = s.trim();
    if s.is_empty() || s == "auto" {
        return 0.0;
    }
    if let Some(num) = s.strip_suffix("px") {
        return num.parse().unwrap_or(0.0);
    }
    s.parse().unwrap_or(0.0)
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
struct BrowserViewport {
    width: f64,
    height: f64,
    device_pixel_ratio: f64,
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    // Window measurements cross the wasm/JS boundary. They only change on a viewport event,
    // never as part of a render frame, so keep one cached sample for the hot path.
    static VIEWPORT_CACHE: std::cell::Cell<Option<BrowserViewport>> = const { std::cell::Cell::new(None) };
}

#[cfg(target_arch = "wasm32")]
fn read_browser_viewport() -> BrowserViewport {
    let Some(window) = web_sys::window() else {
        return BrowserViewport {
            width: 800.0,
            height: 600.0,
            device_pixel_ratio: 1.0,
        };
    };
    let (width, height) = window
        .visual_viewport()
        .map(|vv| (vv.width(), vv.height()))
        .filter(|(width, height)| *width > 0.0 && *height > 0.0)
        .unwrap_or_else(|| {
            let width = window
                .inner_width()
                .ok()
                .and_then(|value| value.as_f64())
                .unwrap_or(800.0);
            let height = window
                .inner_height()
                .ok()
                .and_then(|value| value.as_f64())
                .unwrap_or(600.0);
            (width, height)
        });
    BrowserViewport {
        width,
        height,
        device_pixel_ratio: window.device_pixel_ratio(),
    }
}

#[cfg(target_arch = "wasm32")]
fn browser_viewport() -> BrowserViewport {
    VIEWPORT_CACHE.with(|cache| {
        if let Some(viewport) = cache.get() {
            return viewport;
        }
        let viewport = read_browser_viewport();
        cache.set(Some(viewport));
        viewport
    })
}

#[cfg(target_arch = "wasm32")]
fn invalidate_viewport_cache() {
    VIEWPORT_CACHE.with(|cache| cache.set(None));
}

/// Logical viewport size preferring the visible region (`visualViewport`).
#[cfg(target_arch = "wasm32")]
pub fn visible_logical_size() -> (f64, f64) {
    let viewport = browser_viewport();
    (viewport.width, viewport.height)
}

/// Logical viewport size from the browser window (not `#blade` client box).
#[cfg(target_arch = "wasm32")]
pub fn canvas_logical_size() -> (f64, f64) {
    visible_logical_size()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn canvas_logical_size() -> (f64, f64) {
    (800.0, 600.0)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn visible_logical_size() -> (f64, f64) {
    canvas_logical_size()
}

/// Physical viewport size from the browser window (logical × devicePixelRatio).
#[cfg(target_arch = "wasm32")]
pub fn physical_viewport_size() -> (u32, u32) {
    let viewport = browser_viewport();
    (
        (viewport.width * viewport.device_pixel_ratio).round() as u32,
        (viewport.height * viewport.device_pixel_ratio).round() as u32,
    )
}

#[cfg(target_arch = "wasm32")]
pub fn device_pixel_ratio() -> f64 {
    browser_viewport().device_pixel_ratio
}

#[cfg(not(target_arch = "wasm32"))]
pub fn physical_viewport_size() -> (u32, u32) {
    (800, 600)
}

/// Safe-area insets from CSS custom properties (`--sow-sa*` in the shell template).
#[cfg(target_arch = "wasm32")]
pub fn read_safe_area_insets() -> egui::SafeAreaInsets {
    use std::cell::Cell;
    // `get_computed_style` in `compute_safe_area_insets` forces a synchronous style/layout
    // flush — a Firefox reflow killer — and this runs in the per-frame `apply_to_egui` path.
    // Insets only change when the viewport changes, so cache by logical size and recompute
    // only then. dde7d6f's per-frame web path never touched this; the per-frame call was
    // introduced by the winit-0.31-beta DPI workaround.
    thread_local! {
        static CACHE: Cell<Option<((u32, u32), egui::SafeAreaInsets)>> = Cell::new(None);
    }
    let (w, h) = visible_logical_size();
    let key = (w as u32, h as u32);
    if let Some((k, insets)) = CACHE.with(|c| c.get()) {
        if k == key {
            return insets;
        }
    }
    let insets = compute_safe_area_insets();
    CACHE.with(|c| c.set(Some((key, insets))));
    insets
}

#[cfg(target_arch = "wasm32")]
fn compute_safe_area_insets() -> egui::SafeAreaInsets {
    use egui::epaint::MarginF32;
    let Some(window) = web_sys::window() else {
        return egui::SafeAreaInsets(MarginF32::ZERO);
    };
    let Some(document) = window.document() else {
        return egui::SafeAreaInsets(MarginF32::ZERO);
    };
    let Some(element) = document.document_element() else {
        return egui::SafeAreaInsets(MarginF32::ZERO);
    };
    let Ok(Some(style)) = window.get_computed_style(&element) else {
        return egui::SafeAreaInsets(MarginF32::ZERO);
    };
    let read = |var: &str| -> f32 {
        style
            .get_property_value(var)
            .map(|v| parse_css_px(&v))
            .unwrap_or(0.0)
    };
    egui::SafeAreaInsets(MarginF32 {
        left: read("--sow-sal"),
        top: read("--sow-sat"),
        right: read("--sow-sar"),
        bottom: read("--sow-sab"),
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read_safe_area_insets() -> egui::SafeAreaInsets {
    use egui::epaint::MarginF32;
    egui::SafeAreaInsets(MarginF32::ZERO)
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

/// Request a winit resize when the browser visible viewport changes (URL bar, rotation).
#[cfg(target_arch = "wasm32")]
pub fn install_viewport_listeners() {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let Some(window) = web_sys::window() else {
        return;
    };

    let invalidate = Closure::<dyn FnMut()>::new(invalidate_viewport_cache);
    let callback = invalidate.as_ref().unchecked_ref();
    let _ = window.add_event_listener_with_callback("resize", callback);
    let _ = window.add_event_listener_with_callback("orientationchange", callback);
    invalidate.forget();

    if let Some(vv) = window.visual_viewport() {
        let request_resize = Closure::<dyn FnMut()>::new(|| {
            invalidate_viewport_cache();
            if let Some(window) = web_sys::window() {
                if let Ok(ev) = web_sys::Event::new("resize") {
                    let _ = window.dispatch_event(&ev);
                }
            }
        });
        let callback = request_resize.as_ref().unchecked_ref();
        let _ = vv.add_event_listener_with_callback("resize", callback);
        let _ = vv.add_event_listener_with_callback("scroll", callback);
        request_resize.forget();
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn install_viewport_listeners() {}
