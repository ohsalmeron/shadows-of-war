//! Web canvas / viewport logical size for wasm32.
//!
//! Fullscreen shells (play, CrazyGames: `#blade` fills the window) use
//! `window.innerWidth` / `innerHeight` so live browser resize matches pre-embed
//! behavior. Embedded players (`#blade` smaller than the viewport, e.g. site
//! `#game-stage`) use `#blade` `clientWidth` / `clientHeight`.
//!
//! Drives GPU surface size, egui `screen_rect`, and
//! [`sow_ui::ui::theme::compact_viewport`].

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

/// Canvas client dimensions differ from the viewport by more than this → embedded.
#[cfg(target_arch = "wasm32")]
const EMBED_SLACK_PX: f64 = 48.0;

#[cfg(target_arch = "wasm32")]
fn window_inner_size(window: &web_sys::Window) -> (f64, f64) {
    let w = window.inner_width().unwrap().as_f64().unwrap_or(800.0);
    let h = window.inner_height().unwrap().as_f64().unwrap_or(600.0);
    (w, h)
}

#[cfg(target_arch = "wasm32")]
pub fn canvas_logical_size() -> (f64, f64) {
    let Some(window) = web_sys::window() else {
        return (800.0, 600.0);
    };
    let (inner_w, inner_h) = window_inner_size(&window);

    if let Some(document) = window.document() {
        if let Some(el) = document.get_element_by_id("blade") {
            if let Ok(canvas) = el.dyn_into::<web_sys::HtmlCanvasElement>() {
                let cw = canvas.client_width();
                let ch = canvas.client_height();
                if cw > 0 && ch > 0 {
                    let cw = cw as f64;
                    let ch = ch as f64;
                    if (inner_w - cw).abs() > EMBED_SLACK_PX || (inner_h - ch).abs() > EMBED_SLACK_PX {
                        return (cw, ch);
                    }
                }
            }
        }
    }

    (inner_w, inner_h)
}
