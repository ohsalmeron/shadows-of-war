use crate::process;
use anyhow::{bail, Result};

fn tool_path(env_key: &str, cmd: &str) -> Option<String> {
    if let Ok(p) = std::env::var(env_key) {
        if std::path::Path::new(&p).is_file() {
            return Some(p);
        }
    }
    process::which(cmd)
}

fn missing_tools(names: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter(|name| match **name {
            "cwebp" => tool_path("SOW_CWEBP", "cwebp").is_none(),
            "brotli" => tool_path("SOW_BROTLI", "brotli").is_none(),
            "wasm-bindgen-cli" => {
                process::which("wasm-bindgen").is_none() && !wasm_bindgen_from_cargo().unwrap_or(false)
            }
            "terser or npx" => process::which("terser").is_none() && process::which("npx").is_none(),
            _ => false,
        })
        .map(|s| (*s).to_string())
        .collect()
}

fn bail_missing(missing: Vec<String>) -> Result<()> {
    if missing.is_empty() {
        return Ok(());
    }
    bail!(
        "missing tools: {} (run via ./sow so the Nix shell provides them)",
        missing.join(", ")
    );
}

/// WASM packaging: bindgen, minify, brotli. Used by local/cg/prod/ptr.
pub fn check_wasm_tools() -> Result<()> {
    bail_missing(missing_tools(&[
        "brotli",
        "wasm-bindgen-cli",
        "terser or npx",
    ]))?;
    println!("✅ Build tools OK");
    Ok(())
}

/// CDN boot UI resize — only prod/ptr/cg deploy paths need this.
pub fn check_cdn_tools() -> Result<()> {
    bail_missing(missing_tools(&["cwebp"]))?;
    Ok(())
}

fn wasm_bindgen_from_cargo() -> Result<bool> {
    let home = std::env::var("HOME").unwrap_or_default();
    Ok(std::path::Path::new(&home)
        .join(".cargo/bin/wasm-bindgen")
        .is_file())
}

pub fn cwebp() -> Result<String> {
    tool_path("SOW_CWEBP", "cwebp").map(Ok).unwrap_or_else(|| bail!("cwebp not found"))
}

pub fn brotli() -> Result<String> {
    tool_path("SOW_BROTLI", "brotli")
        .map(Ok)
        .unwrap_or_else(|| bail!("brotli not found"))
}

pub fn wasm_bindgen() -> Result<String> {
    if let Some(p) = process::which("wasm-bindgen") {
        return Ok(p);
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let p = format!("{home}/.cargo/bin/wasm-bindgen");
    if std::path::Path::new(&p).is_file() {
        return Ok(p);
    }
    bail!("wasm-bindgen not found")
}

pub fn terser_cmd() -> Result<Vec<String>> {
    if process::which("terser").is_some() {
        return Ok(vec!["terser".into()]);
    }
    if process::which("npx").is_some() {
        return Ok(vec!["npx".into(), "--yes".into(), "terser".into()]);
    }
    bail!("terser/npx not found")
}

pub fn wasm_opt() -> Option<String> {
    tool_path("SOW_WASM_OPT", "wasm-opt")
}
