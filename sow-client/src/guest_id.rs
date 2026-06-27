//! Stable anonymous visitor ID for players without a platform login.

pub const STORAGE_KEY: &str = "sow_guest_id";

/// Load or create a persistent guest ID (provider `local` in sow-database).
pub fn load_or_create() -> String {
    if let Some(id) = load() {
        return id;
    }
    let id = new_guest_id();
    save(&id);
    id
}

fn new_guest_id() -> String {
    let mut bytes = [0u8; 16];
    let _ = getrandom::getrandom(&mut bytes);
    format!("guest_{:032x}", u128::from_be_bytes(bytes))
}

#[cfg(target_arch = "wasm32")]
fn load() -> Option<String> {
    let storage = web_sys::window()?.local_storage().ok()??;
    storage
        .get_item(STORAGE_KEY)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
}

#[cfg(target_arch = "wasm32")]
fn save(id: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(STORAGE_KEY, id);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load() -> Option<String> {
    let path = guest_id_path();
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
fn save(id: &str) {
    let path = guest_id_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, id);
}

#[cfg(not(target_arch = "wasm32"))]
fn guest_id_path() -> std::path::PathBuf {
    crate::paths::native_data_dir().join(STORAGE_KEY)
}
