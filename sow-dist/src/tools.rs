use crate::process;
use anyhow::{bail, Result};

pub fn check_build_tools() -> Result<()> {
    let mut missing = Vec::new();
    if process::which("cwebp").is_none() {
        missing.push("cwebp");
    }
    if process::which("brotli").is_none() {
        missing.push("brotli");
    }
    if process::which("wasm-bindgen").is_none() && !wasm_bindgen_from_cargo()? {
        missing.push("wasm-bindgen-cli");
    }
    if process::which("terser").is_none() && process::which("npx").is_none() {
        missing.push("terser or npx");
    }
    if !missing.is_empty() {
        bail!("missing tools: {}", missing.join(", "));
    }
    println!("✅ Build tools OK");
    Ok(())
}

fn wasm_bindgen_from_cargo() -> Result<bool> {
    let home = std::env::var("HOME").unwrap_or_default();
    Ok(std::path::Path::new(&home)
        .join(".cargo/bin/wasm-bindgen")
        .is_file())
}

pub fn cwebp() -> Result<String> {
    process::which("cwebp").map(Ok).unwrap_or_else(|| bail!("cwebp not found"))
}

pub fn brotli() -> Result<String> {
    process::which("brotli").map(Ok).unwrap_or_else(|| bail!("brotli not found"))
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
    process::which("wasm-opt")
}
