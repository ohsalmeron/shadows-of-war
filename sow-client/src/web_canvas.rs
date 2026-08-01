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

/// Logical viewport size preferring the visible region (`visualViewport`).
#[cfg(target_arch = "wasm32")]
pub fn visible_logical_size() -> (f64, f64) {
    let Some(window) = web_sys::window() else {
        return (800.0, 600.0);
    };
    if let Some(vv) = window.visual_viewport() {
        let w = vv.width();
        let h = vv.height();
        if w > 0.0 && h > 0.0 {
            return (w, h);
        }
    }
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

    let Some(vv) = window.visual_viewport() else {
        return;
    };

    let bump = Closure::<dyn FnMut()>::new(|| {
        if let Some(window) = web_sys::window() {
            if let Ok(ev) = web_sys::Event::new("resize") {
                let _ = window.dispatch_event(&ev);
            }
        }
    });
    let _ = vv.add_event_listener_with_callback("resize", bump.as_ref().unchecked_ref());
    let _ = vv.add_event_listener_with_callback("scroll", bump.as_ref().unchecked_ref());
    bump.forget();
}

#[cfg(not(target_arch = "wasm32"))]
pub fn install_viewport_listeners() {}
