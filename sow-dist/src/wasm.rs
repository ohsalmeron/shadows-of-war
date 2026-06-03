use crate::paths::Paths;
use crate::process;
use crate::tools;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};

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
    let bindgen = tools::wasm_bindgen()?;
    let wasm_in = paths.wasm_release_input();
    process::run(
        &bindgen,
        &[
            "--out-dir",
            &out_dir.to_string_lossy(),
            "--target",
            "web",
            "--out-name",
            out_name,
            "--no-typescript",
            &wasm_in.to_string_lossy(),
        ],
        Some(&paths.root),
    )?;
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
    let brotli = tools::brotli()?;
    process::run(&brotli, &["-f", "-Z", &path.to_string_lossy()], None)?;
    Ok(())
}

pub fn minify_js(js: &Path) -> Result<()> {
    let mut cmd = tools::terser_cmd()?;
    let mut args: Vec<String> = std::mem::take(&mut cmd);
    args.extend([
        js.to_string_lossy().to_string(),
        "-c".into(),
        "-m".into(),
        "--module".into(),
        "-o".into(),
        format!("{}.min", js.display()),
    ]);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    process::run(refs[0], &refs[1..], None)?;
    fs::rename(format!("{}.min", js.display()), js)?;
    Ok(())
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
