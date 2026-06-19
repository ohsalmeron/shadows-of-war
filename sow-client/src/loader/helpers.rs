#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

/// Empty splash status -> loading screen shows i18n `loading_screen.loading` + progress %.
pub(super) fn splash_show_loading(splash: &mut sow_ui::ui::loading_screen::SplashState) {
    splash.status_text.clear();
}

pub(super) fn splash_show_loading_progress(
    splash: &mut sow_ui::ui::loading_screen::SplashState,
    progress: f32,
) {
    splash_show_loading(splash);
    splash.progress = splash.progress.max(progress);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn hide_web_loader() {
    if let Some(window) = web_sys::window() {
        let _ = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("hideWebLoader"))
            .and_then(|f| {
                if f.is_function() {
                    let func: js_sys::Function = f.unchecked_into();
                    let _ = func.call0(&wasm_bindgen::JsValue::NULL);
                }
                Ok(wasm_bindgen::JsValue::NULL)
            });
    }
}
