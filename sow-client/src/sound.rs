#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = playBackgroundMusic)]
    pub fn play_background_music();

    #[wasm_bindgen(js_name = stopBackgroundMusic)]
    pub fn stop_background_music();

    #[wasm_bindgen(js_name = playSfx)]
    pub fn play_sfx(sfx_name: &str);

    #[wasm_bindgen(js_name = setMuteAll)]
    pub fn set_mute_all(mute: bool);
}

// Fallbacks for Desktop / Android platforms (so the engine compiles successfully on all targets)
#[cfg(not(target_arch = "wasm32"))]
pub fn play_background_music() {}

#[cfg(not(target_arch = "wasm32"))]
pub fn stop_background_music() {}

#[cfg(not(target_arch = "wasm32"))]
pub fn play_sfx(_sfx_name: &str) {}

#[cfg(not(target_arch = "wasm32"))]
pub fn set_mute_all(_mute: bool) {}
