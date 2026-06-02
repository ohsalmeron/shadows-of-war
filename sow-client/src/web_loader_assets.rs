use sow_ui::ui::asset_loader::{AssetLoader, UiSplashTexture};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
fn parse_texture_entry(val: &wasm_bindgen::JsValue) -> Option<(u32, u32, Vec<u8>)> {
    let width = js_sys::Reflect::get(val, &wasm_bindgen::JsValue::from_str("width"))
        .ok()?
        .as_f64()? as u32;
    let height = js_sys::Reflect::get(val, &wasm_bindgen::JsValue::from_str("height"))
        .ok()?
        .as_f64()? as u32;
    let rgba_val = js_sys::Reflect::get(val, &wasm_bindgen::JsValue::from_str("rgba")).ok()?;
    let arr = js_sys::Uint8Array::from(rgba_val);
    let mut bytes = vec![0u8; arr.length() as usize];
    arr.copy_to(&mut bytes);
    Some((width, height, bytes))
}

#[cfg(target_arch = "wasm32")]
fn ingest_entry(
    ctx: &egui::Context,
    asset_loader: &mut AssetLoader,
    exported: &wasm_bindgen::JsValue,
    key: &str,
    kind: UiSplashTexture,
) -> bool {
    let entry = match js_sys::Reflect::get(exported, &wasm_bindgen::JsValue::from_str(key)) {
        Ok(v) if !v.is_null() && !v.is_undefined() => v,
        _ => return false,
    };
    let Some((width, height, rgba)) = parse_texture_entry(&entry) else {
        log::warn!("Failed to parse web loader texture entry: {key}");
        return false;
    };
    asset_loader.ingest_ui_splash_texture(ctx, kind, width, height, &rgba)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn try_ingest_web_loader_textures(
    ctx: &egui::Context,
    asset_loader: &mut AssetLoader,
) -> bool {
    if asset_loader.ui_splash_ready() {
        return true;
    }

    let Some(window) = web_sys::window() else {
        return false;
    };

    let exported = match js_sys::Reflect::get(
        &window,
        &wasm_bindgen::JsValue::from_str("exportWebLoaderTextures"),
    ) {
        Ok(f) if f.is_function() => {
            let func: js_sys::Function = f.unchecked_into();
            match func.call0(&wasm_bindgen::JsValue::NULL) {
                Ok(v) if !v.is_null() && !v.is_undefined() => v,
                _ => {
                    log::warn!("exportWebLoaderTextures returned no data");
                    return false;
                }
            }
        }
        _ => {
            log::warn!("exportWebLoaderTextures is not available");
            return false;
        }
    };

    let _ = ingest_entry(
        ctx,
        asset_loader,
        &exported,
        "loader_empty",
        UiSplashTexture::LoaderEmpty,
    );
    let _ = ingest_entry(
        ctx,
        asset_loader,
        &exported,
        "loader_full",
        UiSplashTexture::LoaderFull,
    );
    let _ = ingest_entry(
        ctx,
        asset_loader,
        &exported,
        "splash_desktop",
        UiSplashTexture::SplashDesktop,
    );
    let _ = ingest_entry(
        ctx,
        asset_loader,
        &exported,
        "splash_mobile",
        UiSplashTexture::SplashMobile,
    );

    if asset_loader.ui_splash_ready() {
        log::info!("Ingested web boot loader textures for enter/exit splash");
        true
    } else {
        log::warn!("Web boot loader texture ingest incomplete");
        false
    }
}

/// Max frames to wait for HTML loader images before leaving boot without full splash textures.
#[cfg(target_arch = "wasm32")]
const BOOT_INGEST_MAX_WAIT_FRAMES: u32 = 300;

/// Try ingest each frame until all four splash textures are ready or timeout.
/// Returns `true` when boot may proceed to main menu and hide the HTML loader.
#[cfg(target_arch = "wasm32")]
pub(crate) fn ensure_boot_web_loader_textures(
    ctx: &egui::Context,
    asset_loader: &mut AssetLoader,
    wait_frames: &mut u32,
) -> bool {
    if asset_loader.ui_splash_ready() {
        return true;
    }

    try_ingest_web_loader_textures(ctx, asset_loader);

    if asset_loader.ui_splash_ready() {
        return true;
    }

    *wait_frames = wait_frames.saturating_add(1);
    if *wait_frames >= BOOT_INGEST_MAX_WAIT_FRAMES {
        log::warn!(
            "Web boot loader texture ingest timed out after {} frames",
            BOOT_INGEST_MAX_WAIT_FRAMES
        );
        return true;
    }

    false
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn ensure_splash_textures_from_web_loader(
    ctx: &egui::Context,
    asset_loader: &mut AssetLoader,
) {
    if asset_loader.ui_splash_ready() {
        return;
    }
    try_ingest_web_loader_textures(ctx, asset_loader);
}
