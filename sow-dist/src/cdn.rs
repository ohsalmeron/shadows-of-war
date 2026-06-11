use crate::config::DeployConfig;
use crate::paths::Paths;
use anyhow::{bail, Context, Result};
use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use image::ExtendedColorType;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

const LEADERS: &[&str] = &[
    "caesar",
    "cleopatra",
    "ragnar",
    "sun_tzu",
    "alexander",
    "genghis_khan",
    "richard_the_lionheart",
    "vercingetorix",
    "boudica",
    "lady_six_sky",
    "leonidas",
    "napoleon",
];

/// Phase 1: local CDN prep (resize boot UI webp into assets/cdn/ui).
pub fn prepare(paths: &Paths) -> Result<()> {
    prepare_boot_ui(paths)?;
    check_leader_portraits(&paths.assets_cdn.join("leaders"))?;
    check_avatar_files(&paths.assets_cdn.join("avatars"))?;
    Ok(())
}

/// Phase 3: rsync assets/cdn/ to marketing host. Returns true if rsync ran.
pub fn ship_or_skip(paths: &Paths, cfg: &DeployConfig) -> Result<bool> {
    let hash = cdn_input_hash(paths)?;
    let cache = paths.cdn_hash_cache();
    let cached = fs::read_to_string(&cache)
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if cached == hash {
        println!("==> CDN unchanged — skipping ship");
        return Ok(false);
    }

    let gcp = cfg.gcp();
    let assets_path = cfg.prod_assets_path();
    println!("==> Syncing CDN → {assets_path}/cdn/");
    gcp.run_remote(&format!(
        "mkdir -p {assets_path}/cdn/leaders {assets_path}/cdn/ui {assets_path}/cdn/avatars"
    ))?;
    gcp.rsync_dir_with_opts(
        &paths.dist_root(),
        &paths.assets_cdn.display().to_string(),
        &format!("{assets_path}/cdn"),
        &["-avz", "--chmod=Du=rwx,Dgo=rx,Fu=rw,Fgo=r"],
    )?;
    gcp.run_remote(&format!("chmod -R a+rX {assets_path}/cdn"))?;
    gcp.run_remote(&format!("sudo restorecon -R {assets_path}/cdn"))?;

    if let Some(parent) = cache.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&cache, format!("{hash}\n"))?;
    println!("✅ CDN shipped");
    Ok(true)
}

/// Phase 4: HTTP verify prod CDN assets.
pub fn verify_prod_cdn(cfg: &DeployConfig) -> Result<()> {
    println!("==> Verifying prod CDN...");
    let base = cfg.site_url();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let urls = [
        format!("{base}/assets/cdn/leaders/caesar_desktop.webp"),
        format!("{base}/assets/cdn/ui/loader_empty.webp"),
        format!("{base}/assets/cdn/avatars/caesar.webp"),
    ];
    for url in urls {
        let resp = client.head(&url).send().with_context(|| url.clone())?;
        if !resp.status().is_success() {
            bail!("CDN verify failed: {url} → {}", resp.status());
        }
    }
    println!("✅ prod CDN assets OK");
    Ok(())
}

pub fn cdn_input_hash(paths: &Paths) -> Result<String> {
    let mut h = Sha256::new();
    hash_cdn_dir(&mut h, &paths.assets_cdn)?;
    Ok(format!("{:x}", h.finalize()))
}

fn hash_cdn_dir(h: &mut Sha256, dir: &Path) -> Result<()> {
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
            hash_cdn_dir(h, &path)?;
        } else if path.is_file() {
            h.update(fs::read(&path).with_context(|| path.display().to_string())?);
        }
    }
    Ok(())
}

fn prepare_boot_ui(paths: &Paths) -> Result<()> {
    let ui_src = paths.assets_static.join("ui");
    let ui_cdn = paths.assets_cdn.join("ui");
    fs::create_dir_all(&ui_cdn)?;
    if !ui_src.join("loader_empty.webp").is_file() {
        if ui_cdn.join("loader_empty.webp").is_file() {
            println!("==> Using existing assets/cdn/ui/");
            return Ok(());
        }
        bail!("missing assets/static/ui/loader_empty.webp and assets/cdn/ui/");
    }
    for (src, w, h, out) in [
        ("loader_empty.webp", 1032, 256, "loader_empty.webp"),
        ("loader_full.webp", 1032, 256, "loader_full.webp"),
        (
            "sow-splash-mobile.webp",
            720,
            1280,
            "sow-splash-mobile.webp",
        ),
    ] {
        resize_webp(&ui_src.join(src), &ui_cdn.join(out), w, h)?;
    }
    fs::copy(
        ui_src.join("sow-splash-desktop.webp"),
        ui_cdn.join("sow-splash-desktop.webp"),
    )?;
    Ok(())
}

fn resize_webp(src: &Path, dst: &Path, width: u32, height: u32) -> Result<()> {
    let img = image::open(src).with_context(|| format!("open {}", src.display()))?;
    let resized = img.resize_exact(width, height, FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    let mut out = Vec::new();
    WebPEncoder::new_lossless(&mut out)
        .encode(
            rgba.as_raw(),
            rgba.width(),
            rgba.height(),
            ExtendedColorType::Rgba8,
        )
        .with_context(|| format!("webp encode {}", dst.display()))?;
    fs::write(dst, out).with_context(|| format!("write {}", dst.display()))?;
    Ok(())
}

fn check_avatar_files(dir: &Path) -> Result<()> {
    let mut missing = Vec::new();
    for slug in LEADERS {
        let p = dir.join(format!("{slug}.webp"));
        if !p.is_file() {
            missing.push(p);
        }
    }
    let null_p = dir.join("null.webp");
    if !null_p.is_file() {
        missing.push(null_p);
    }
    if !missing.is_empty() {
        for p in &missing {
            eprintln!("❌ missing {}", p.display());
        }
        bail!("avatar webps incomplete");
    }
    Ok(())
}

fn check_leader_portraits(dir: &Path) -> Result<()> {
    let mut missing = Vec::new();
    for slug in LEADERS {
        for form in ["desktop", "mobile"] {
            let p = dir.join(format!("{slug}_{form}.webp"));
            if !p.is_file() {
                missing.push(p);
            }
        }
    }
    if !missing.is_empty() {
        for p in &missing {
            eprintln!("❌ missing {}", p.display());
        }
        bail!("leader portraits incomplete");
    }
    Ok(())
}
