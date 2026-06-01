//! Trigger browser file downloads for map editor export (WASM).

#[cfg(target_arch = "wasm32")]
pub fn install_wasm_map_export_hook() {
    sow_map::wasm_export::set_export_hook(trigger_browser_download);
}

#[cfg(target_arch = "wasm32")]
fn trigger_browser_download(filename: &str, data: &[u8]) {
    use wasm_bindgen::JsCast;
    let window = match web_sys::window() {
        Some(w) => w,
        None => {
            log::error!("map export: no window");
            return;
        }
    };
    let document = match window.document() {
        Some(d) => d,
        None => {
            log::error!("map export: no document");
            return;
        }
    };

    let array = js_sys::Uint8Array::from(data);
    let blob_parts = js_sys::Array::new();
    blob_parts.push(&array);
    let blob = match web_sys::Blob::new_with_u8_array_sequence(&blob_parts) {
        Ok(b) => b,
        Err(e) => {
            log::error!("map export: blob failed: {e:?}");
            return;
        }
    };

    let url = match web_sys::Url::create_object_url_with_blob(&blob) {
        Ok(u) => u,
        Err(e) => {
            log::error!("map export: object URL failed: {e:?}");
            return;
        }
    };

    let anchor = match document.create_element("a") {
        Ok(e) => e,
        Err(e) => {
            log::error!("map export: anchor element failed: {e:?}");
            let _ = web_sys::Url::revoke_object_url(&url);
            return;
        }
    };
    let anchor: web_sys::HtmlAnchorElement = match anchor.dyn_into() {
        Ok(a) => a,
        Err(_) => {
            let _ = web_sys::Url::revoke_object_url(&url);
            return;
        }
    };
    anchor.set_href(&url);
    anchor.set_download(filename);
    let _ = anchor.style().set_property("display", "none");
    if document.body().is_none() {
        log::error!("map export: no document body");
        let _ = web_sys::Url::revoke_object_url(&url);
        return;
    }
    let body = document.body().unwrap();
    let _ = body.append_child(&anchor);
    anchor.click();
    let _ = body.remove_child(&anchor);
    let _ = web_sys::Url::revoke_object_url(&url);
    log::info!("map export: downloaded {filename} ({} bytes)", data.len());
}

#[cfg(not(target_arch = "wasm32"))]
pub fn install_wasm_map_export_hook() {}
