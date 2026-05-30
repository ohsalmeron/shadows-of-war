//! Partner portal SDK hooks (Poki / CrazyGames). No-op when JS helpers are absent.

#[cfg(target_arch = "wasm32")]
fn call_window_hook(name: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(val) = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str(name)) else {
        return;
    };
    if val.is_function() {
        let Ok(func) = val.dyn_into::<js_sys::Function>() else {
            return;
        };
        let _ = func.call0(&wasm_bindgen::JsValue::NULL);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn call_window_hook(_name: &str) {}

pub fn gameplay_start() {
    call_window_hook("SOW_portalGameplayStart");
}

pub fn gameplay_stop() {
    call_window_hook("SOW_portalGameplayStop");
}

pub fn load_stop() {
    call_window_hook("SOW_portalLoadStop");
}
