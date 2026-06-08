use crate::paths::Paths;
use crate::process;
use crate::tools;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use wasm_bindgen_cli_support::Bindgen;

fn run_command(mut c: Command) -> Result<()> {
    c.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().context("spawn cargo")?;
    stream(child.stdout.take());
    stream(child.stderr.take());
    if !child.wait()?.success() {
        anyhow::bail!("cargo failed");
    }
    Ok(())
}

fn stream(pipe: Option<impl Read>) {
    if let Some(out) = pipe {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            println!("{line}");
        }
    }
}

const WASM_OPT_TAG: &str = "oz-v1";

pub fn compile(paths: &Paths) -> Result<()> {
    println!("==> Compiling WASM (wasm-release)...");
    let mut c = std::process::Command::new("cargo");
    c.current_dir(&paths.root)
        .env("RUSTFLAGS", "-C target-feature=-bulk-memory")
        .args([
            "build",
            "--profile",
            "wasm-release",
            "-p",
            "sow-client",
            "--target",
            "wasm32-unknown-unknown",
        ]);
    run_command(c)?;
    let wasm = paths.wasm_release_input();
    if !wasm.is_file() {
        anyhow::bail!("missing {}", wasm.display());
    }
    Ok(())
}

pub fn bindgen(paths: &Paths, out_dir: &Path, out_name: &str) -> Result<()> {
    Bindgen::new()
        .input_path(paths.wasm_release_input())
        .web(true)
        .context("wasm-bindgen web target")?
        .out_name(out_name)
        .typescript(false)
        .generate(out_dir)
        .context("wasm-bindgen generate")?;
    Ok(())
}

pub fn optimize_wasm(paths: &Paths, wasm_path: &Path) -> Result<()> {
    let hash = file_sha256(wasm_path)?;
    let cache_dir = &paths.wasm_opt_cache;
    fs::create_dir_all(cache_dir)?;
    let cache_path = cache_dir.join(format!("{WASM_OPT_TAG}-{hash}.wasm"));
    if cache_path.is_file() {
        println!("==> wasm-opt cache hit");
        fs::copy(&cache_path, wasm_path)?;
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
        println!("✅ wasm-opt finished");
    } else {
        println!("⚠️  wasm-opt not found — skipping");
    }
    Ok(())
}

pub fn brotli_file(path: &Path) -> Result<()> {
    let input = fs::read(path)?;
    let compressed = brotli_compress(&input)?;
    fs::write(format!("{}.br", path.display()), compressed)?;
    Ok(())
}

pub fn minify_js(js: &Path) -> Result<()> {
    let source = fs::read_to_string(js)?;
    let minified = minifier::js::minify(&source).to_string();
    fs::write(js, minified)?;
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

fn file_sha256(path: &Path) -> Result<String> {
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
