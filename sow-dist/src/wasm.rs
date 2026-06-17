use crate::paths::Paths;
use crate::process;
use crate::tools;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use wasm_bindgen_cli_support::Bindgen;

const WASM_OPT_TAG: &str = "oz-v1";

pub fn compile(paths: &Paths) -> Result<()> {
    if process::check_any_cargo_lock(&paths.cargo_target) {
        println!("==> Cargo target directory is locked by another process. Waiting for lock...");
    }
    println!("==> Compiling WASM (wasm-release)...");
    process::run_env(
        "cargo",
        &[
            "build",
            "--profile",
            "wasm-release",
            "-p",
            "sow-client",
            "--target",
            "wasm32-unknown-unknown",
        ],
        Some(&paths.root),
        &[("RUSTFLAGS", "-C target-feature=-bulk-memory")],
    )?;
    let wasm = paths.wasm_release_input();
    if !wasm.is_file() {
        anyhow::bail!("missing {}", wasm.display());
    }
    Ok(())
}

pub fn bindgen(paths: &Paths, out_dir: &Path, out_name: &str) -> Result<()> {
    println!("==> Running wasm-bindgen for {out_name}...");
    Bindgen::new()
        .input_path(paths.wasm_release_input())
        .web(true)
        .context("wasm-bindgen web target")?
        .out_name(out_name)
        .typescript(false)
        .generate(out_dir)
        .context("wasm-bindgen generate")?;
    println!("✅ wasm-bindgen finished");
    Ok(())
}

pub fn optimize_wasm(paths: &Paths, wasm_path: &Path) -> Result<()> {
    let hash = file_sha256(wasm_path)?;
    let cache_dir = &paths.wasm_opt_cache;
    fs::create_dir_all(cache_dir)?;
    let cache_path = cache_dir.join(format!("{WASM_OPT_TAG}-{hash}.wasm"));
    let cache_path_br = cache_dir.join(format!("{WASM_OPT_TAG}-{hash}.wasm.br"));

    if cache_path.is_file() {
        println!("==> wasm-opt cache hit");
        fs::copy(&cache_path, wasm_path)?;
        if cache_path_br.is_file() {
            let br_dest = format!("{}.br", wasm_path.display());
            fs::copy(&cache_path_br, br_dest)?;
        }
        return Ok(());
    }
    if let Some(opt) = tools::wasm_opt() {
        println!("==> wasm-opt -Oz...");
        process::run(
            &opt,
            &[
                "-Oz",
                "--strip-debug",
                "--vacuum",
                "--enable-bulk-memory",
                "--enable-nontrapping-float-to-int",
                &wasm_path.to_string_lossy(),
                "-o",
                &wasm_path.to_string_lossy(),
            ],
            None,
        )?;
        fs::copy(wasm_path, &cache_path)?;

        // Also pre-compress the optimized WASM to cache
        let input = fs::read(wasm_path)?;
        let compressed = brotli_compress(&input)?;
        fs::write(&cache_path_br, compressed)?;

        println!("✅ wasm-opt finished");
    } else {
        println!("⚠️  wasm-opt not found — skipping");
    }
    Ok(())
}

pub fn brotli_file(path: &Path) -> Result<()> {
    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let br_path = std::path::PathBuf::from(format!("{}.br", path.display()));
    if br_path.is_file() {
        println!("==> Brotli compression skipped (cache hit for {filename})");
        return Ok(());
    }
    println!("==> Brotli compressing {filename}...");
    let input = fs::read(path)?;
    let compressed = brotli_compress(&input)?;
    fs::write(br_path, compressed)?;
    println!("✅ Brotli compressing {filename} finished");
    Ok(())
}

pub fn minify_js(js: &Path) -> Result<()> {
    let filename = js.file_name().unwrap_or_default().to_string_lossy();
    println!("==> Minifying {filename}...");
    let source = fs::read_to_string(js)?;
    let minified = minifier::js::minify(&source).to_string();
    fs::write(js, minified)?;
    println!("✅ Minifying {filename} finished");
    Ok(())
}

fn brotli_compress(input: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut writer = brotli::CompressorWriter::new(&mut out, 4096, 11, 22);
    writer.write_all(input)?;
    writer.flush()?;
    drop(writer);
    Ok(out)
}

pub fn file_sha256(path: &Path) -> Result<String> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
