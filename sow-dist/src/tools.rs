use crate::process;

/// Optional `wasm-opt` from Nix (`SOW_WASM_OPT` / `binaryen`).
pub fn wasm_opt() -> Option<String> {
    if let Ok(p) = std::env::var("SOW_WASM_OPT") {
        if std::path::Path::new(&p).is_file() {
            return Some(p);
        }
    }
    process::which("wasm-opt")
}
