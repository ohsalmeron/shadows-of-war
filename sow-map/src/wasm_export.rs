//! Browser download hook for map editor export (WASM only). Set from `sow-client` at startup.

use std::sync::OnceLock;

static EXPORT_HOOK: OnceLock<fn(&str, &[u8])> = OnceLock::new();

/// Register the function that triggers a file download in the browser (`filename`, raw bytes).
pub fn set_export_hook(hook: fn(&str, &[u8])) {
    let _ = EXPORT_HOOK.set(hook);
}

/// Deliver compiled map artifacts to the browser (no-op if hook was not registered).
pub fn trigger_download(filename: &str, data: &[u8]) {
    if let Some(hook) = EXPORT_HOOK.get() {
        hook(filename, data);
    } else {
        log::warn!(
            "WASM map export hook not set; cannot download {filename} ({} bytes)",
            data.len()
        );
    }
}
