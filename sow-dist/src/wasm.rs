use crate::paths::Paths;
use crate::process;
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use wasm_bindgen_cli_support::Bindgen;
use wasm_opt::{Feature, OptimizationOptions, Pass};

const WASM_OPT_TAG: &str = "oz-v1";

pub fn compile(paths: &Paths, dev: bool) -> Result<()> {
    process::wait_for_cargo_unlock(&paths.cargo_target);
    println!("==> Compiling WASM (wasm-release)...");
    let mut args = vec![
        "build",
        "--profile",
        "wasm-release",
        "-p",
        "sow-client",
        "--target",
        "wasm32-unknown-unknown",
    ];
    if dev {
        // Local QA builds compile in the dev sidebar / dev tooling; prod never does.
        args.push("--features");
        args.push("dev");
        println!("==> (local) dev tools enabled");
    }
    process::run_env(
        "cargo",
        &args,
        Some(&paths.root),
        // Keep bulk-memory DISABLED for the wasm32 build: enabling it makes rustc emit
        // passive data segments + memory.init that break instantiation in the target
        // Firefox/WebGL runtime (works on native, dies on web — even the main menu).
        // Long-standing known-good value; wasm-opt still handles BulkMemory downstream.
        &[("RUSTFLAGS", "-C target-feature=-bulk-memory")],
    )?;
    let wasm = paths.wasm_release_input();
    if !wasm.is_file() {
        bail!("WASM compile produced no artifact: {}", wasm.display());
    }
    let bytes = fs::read(&wasm).with_context(|| format!("read {}", wasm.display()))?;
    if bytes.is_empty() {
        bail!("WASM compile produced empty artifact: {}", wasm.display());
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
    require_nonempty_file(wasm_path, "wasm-opt input")?;

    let hash = file_sha256(wasm_path)?;
    let cache_dir = &paths.wasm_opt_cache;
    fs::create_dir_all(cache_dir).with_context(|| format!("create {}", cache_dir.display()))?;
    let cache_path = cache_dir.join(format!("{WASM_OPT_TAG}-{hash}.wasm"));
    let cache_path_br = cache_dir.join(format!("{WASM_OPT_TAG}-{hash}.wasm.br"));
    let br_dest = brotli_sidecar_path(wasm_path);

    if cache_path.is_file() {
        println!("==> wasm-opt cache hit ({})", cache_path.display());
        fs::copy(&cache_path, wasm_path)
            .with_context(|| format!("restore wasm-opt cache → {}", wasm_path.display()))?;
        require_nonempty_file(wasm_path, "wasm-opt cached output")?;
        if cache_path_br.is_file() {
            fs::copy(&cache_path_br, &br_dest).with_context(|| {
                format!("restore wasm-opt brotli cache → {}", br_dest.display())
            })?;
        } else {
            write_brotli_sidecar(wasm_path, &br_dest)?;
            fs::copy(&br_dest, &cache_path_br).with_context(|| {
                format!("write wasm-opt brotli cache {}", cache_path_br.display())
            })?;
        }
        require_brotli_sidecar(wasm_path)?;
        return Ok(());
    }

    println!("==> wasm-opt -Oz ({})...", wasm_path.display());
    let parent = wasm_path
        .parent()
        .context("wasm path has no parent directory")?;
    let tmp =
        tempfile::NamedTempFile::new_in(parent).context("create wasm-opt temp output file")?;
    run_wasm_opt(wasm_path, tmp.path())?;
    tmp.persist(wasm_path).map_err(|e| {
        anyhow::anyhow!(
            "wasm-opt could not replace {}: {}",
            wasm_path.display(),
            e.error
        )
    })?;
    // NamedTempFile is created 0600; persist() keeps those perms. Fix so nginx can read it.
    fs::set_permissions(wasm_path, fs::Permissions::from_mode(0o644))
        .with_context(|| format!("chmod 0644 {}", wasm_path.display()))?;
    require_nonempty_file(wasm_path, "wasm-opt output")?;

    fs::copy(wasm_path, &cache_path)
        .with_context(|| format!("write wasm-opt cache {}", cache_path.display()))?;

    write_brotli_sidecar(wasm_path, &br_dest)?;
    fs::copy(&br_dest, &cache_path_br)
        .with_context(|| format!("write wasm-opt brotli cache {}", cache_path_br.display()))?;
    require_brotli_sidecar(wasm_path)?;

    println!("✅ wasm-opt finished");
    Ok(())
}

fn run_wasm_opt(in_path: &Path, out_path: &Path) -> Result<()> {
    let mut opts = OptimizationOptions::new_optimize_for_size_aggressively();
    opts.enable_feature(Feature::BulkMemory);
    opts.enable_feature(Feature::TruncSat);
    opts.debug_info(false);
    opts.add_pass(Pass::Vacuum);
    opts.run(in_path, out_path).map_err(|e| {
        anyhow::anyhow!(
            "wasm-opt failed for {} → {}: {e}",
            in_path.display(),
            out_path.display()
        )
    })?;
    require_nonempty_file(out_path, "wasm-opt temp output")?;
    Ok(())
}

pub fn brotli_file(path: &Path) -> Result<()> {
    require_nonempty_file(path, "brotli source")?;
    let br_path = brotli_sidecar_path(path);
    let filename = path.file_name().unwrap_or_default().to_string_lossy();

    if br_path.is_file() {
        match fs::metadata(&br_path) {
            Ok(meta) if meta.len() > 0 => {
                println!("==> Brotli cache hit for {filename} ({} bytes)", meta.len());
                return Ok(());
            }
            Ok(_) => {
                println!("==> Brotli sidecar empty — recompressing {filename}...");
            }
            Err(e) => {
                bail!("brotli sidecar unreadable {}: {e}", br_path.display());
            }
        }
    }

    write_brotli_sidecar(path, &br_path)?;
    require_brotli_sidecar(path)?;
    let out_len = fs::metadata(&br_path)?.len();
    println!(
        "✅ Brotli {filename} → {} ({out_len} bytes)",
        br_path.display()
    );
    Ok(())
}

pub fn require_brotli_sidecar(path: &Path) -> Result<()> {
    let br_path = brotli_sidecar_path(path);
    if !br_path.is_file() {
        bail!(
            "brotli sidecar missing for {} (expected {})",
            path.display(),
            br_path.display()
        );
    }
    let len = fs::metadata(&br_path)
        .with_context(|| format!("stat brotli sidecar {}", br_path.display()))?
        .len();
    if len == 0 {
        bail!("brotli sidecar empty: {}", br_path.display());
    }
    Ok(())
}

fn write_brotli_sidecar(src: &Path, br_path: &Path) -> Result<()> {
    let filename = src.file_name().unwrap_or_default().to_string_lossy();
    let input = fs::read(src).with_context(|| format!("brotli read {}", src.display()))?;
    if input.is_empty() {
        bail!("brotli source empty: {}", src.display());
    }
    println!("==> Brotli compressing {filename}...");
    let compressed = brotli_compress(&input).with_context(|| {
        format!(
            "brotli compression failed for {} ({} bytes input)",
            src.display(),
            input.len()
        )
    })?;
    if compressed.is_empty() {
        bail!(
            "brotli produced empty output for {} ({} bytes input)",
            src.display(),
            input.len()
        );
    }
    fs::write(br_path, &compressed).with_context(|| format!("write {}", br_path.display()))?;
    Ok(())
}

pub fn minify_js(js: &Path) -> Result<()> {
    require_nonempty_file(js, "minify input")?;
    let filename = js.file_name().unwrap_or_default().to_string_lossy();
    println!("==> Minifying {filename}...");
    let source =
        fs::read_to_string(js).with_context(|| format!("read JS for minify {}", js.display()))?;
    if source.trim().is_empty() {
        bail!("minify input empty: {}", js.display());
    }
    let minified = minifier::js::minify(&source).to_string();
    if minified.is_empty() {
        bail!("minify produced empty output: {}", js.display());
    }
    fs::write(js, minified).with_context(|| format!("write minified {}", js.display()))?;
    println!("✅ Minifying {filename} finished");
    Ok(())
}

fn brotli_sidecar_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.br", path.display()))
}

fn require_nonempty_file(path: &Path, label: &str) -> Result<()> {
    if !path.is_file() {
        bail!("{label} missing: {}", path.display());
    }
    let len = fs::metadata(path)
        .with_context(|| format!("stat {label} {}", path.display()))?
        .len();
    if len == 0 {
        bail!("{label} empty: {}", path.display());
    }
    Ok(())
}

fn brotli_compress(input: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut writer = brotli::CompressorWriter::new(&mut out, 4096, 11, 22);
    writer.write_all(input).context("brotli compressor write")?;
    writer.flush().context("brotli compressor flush")?;
    drop(writer);
    Ok(out)
}

pub fn file_sha256(path: &Path) -> Result<String> {
    let mut f =
        fs::File::open(path).with_context(|| format!("open for sha256 {}", path.display()))?;
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
