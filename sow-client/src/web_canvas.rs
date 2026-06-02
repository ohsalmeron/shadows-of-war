//! Web canvas layout size — used instead of `window.innerWidth/Height` so embedded
//! `#blade` (CSS-sized to `#game-stage`) matches the WASM render surface.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
pub fn canvas_logical_size() -> (f64, f64) {
    let Some(window) = web_sys::window() else {
        return (800.0, 600.0);
    };
    if let Some(document) = window.document() {
        if let Some(el) = document.get_element_by_id("blade") {
            if let Ok(canvas) = el.dyn_into::<web_sys::HtmlCanvasElement>() {
                let w = canvas.client_width();
                let h = canvas.client_height();
                if w > 0 && h > 0 {
                    return (w as f64, h as f64);
                }
            }
        }
    }
    let w = window.inner_width().unwrap().as_f64().unwrap_or(800.0);
    let h = window.inner_height().unwrap().as_f64().unwrap_or(600.0);
    (w, h)
}
