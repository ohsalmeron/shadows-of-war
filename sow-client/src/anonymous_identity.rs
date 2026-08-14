//! Canonical anonymous account identifier for the browser/native client.
//!
//! The value stored here is the server's canonical `account_id`.

pub const ACCOUNT_ID_STORAGE_KEY: &str = "sow_account_id";

pub fn load_account_id() -> Option<String> {
    load_storage(ACCOUNT_ID_STORAGE_KEY)
}

pub fn save_account_id(account_id: &str) {
    save_storage(ACCOUNT_ID_STORAGE_KEY, account_id);
}

pub fn clear_account_id() {
    clear_storage(ACCOUNT_ID_STORAGE_KEY);
}

#[cfg(target_arch = "wasm32")]
fn load_storage(key: &str) -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()??
        .get_item(key)
        .ok()
        .flatten()
        .filter(|value| !value.is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
fn load_storage(key: &str) -> Option<String> {
    std::fs::read_to_string(crate::paths::native_data_dir().join(key))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(target_arch = "wasm32")]
fn save_storage(key: &str, value: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(key, value);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_storage(key: &str, value: &str) {
    let path = crate::paths::native_data_dir().join(key);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, value);
}

#[cfg(target_arch = "wasm32")]
fn clear_storage(key: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.remove_item(key);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn clear_storage(key: &str) {
    let _ = std::fs::remove_file(crate::paths::native_data_dir().join(key));
}
