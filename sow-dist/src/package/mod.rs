use crate::config::DeployConfig;
use crate::paths::Paths;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

mod build;
mod verify;

#[derive(Clone, Copy, Debug)]
pub enum Profile {
    SelfHosted,
    Crazygames,
    /// Local marketing iframe embed: raw wasm/js in www/game/, prod APIs, no brotli/deploy/CDN.
    SiteDev,
}

pub fn profile_cache_key(profile: Profile) -> &'static str {
    match profile {
        Profile::SelfHosted => "play",
        Profile::Crazygames => "crazygames",
        Profile::SiteDev => "site-dev",
    }
}

/// Phase 2: package dist output, skipping when WASM + shell inputs are unchanged.
pub fn build_or_skip(
    paths: &Paths,
    profile: Profile,
    out_dir: &Path,
    version: &str,
    cfg: &DeployConfig,
) -> Result<()> {
    let hash = package_input_hash(paths, profile, version)?;
    let cache = paths.package_hash_cache(profile_cache_key(profile));
    let cached = fs::read_to_string(&cache)
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if cached == hash && verify_layout(out_dir, profile).is_ok() {
        println!("==> Package unchanged — skipping ({})", out_dir.display());
        return Ok(());
    }
    build(paths, profile, out_dir, version, cfg)?;
    if let Some(parent) = cache.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&cache, format!("{hash}\n"))?;
    Ok(())
}

fn package_input_hash(paths: &Paths, profile: Profile, version: &str) -> Result<String> {
    let mut h = Sha256::new();
    h.update(profile_cache_key(profile).as_bytes());
    h.update(version.as_bytes());
    let wasm = paths.wasm_release_input();
    if wasm.is_file() {
        h.update(crate::wasm::file_sha256(&wasm)?.as_bytes());
    }
    hash_tree_dir(
        &mut h,
        &paths.shell,
        &["rs", "toml", "js", "html", "template", "svg"],
    )?;
    Ok(format!("{:x}", h.finalize()))
}

fn hash_tree_dir(h: &mut Sha256, dir: &Path, exts: &[&str]) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            hash_tree_dir(h, &path, exts)?;
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| exts.contains(&e))
        {
            h.update(fs::read(&path).with_context(|| path.display().to_string())?);
        }
    }
    Ok(())
}

pub use build::build;
pub use verify::{stage_site_www, verify_layout};
