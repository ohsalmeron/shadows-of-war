use crate::paths::{Paths, PROD_ASSETS_PATH, PROD_HOST, PROD_USER};
use crate::process;
use crate::tools;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

const LEADERS: &[&str] = &[
    "caesar", "cleopatra", "ragnar", "sun_tzu", "alexander", "genghis_khan",
    "richard_the_lionheart", "vercingetorix", "boudica", "lady_six_sky", "leonidas",
    "napoleon",
];

/// Rsync assets/cdn/ to the marketing host (no assets/static/ to VPS).
pub fn sync_to_prod(paths: &Paths) -> Result<()> {
    prepare_boot_ui(paths)?;
    check_leader_portraits(&paths.assets_cdn.join("leaders"))?;
    check_avatar_files(&paths.assets_cdn.join("avatars"))?;
    println!("==> Syncing CDN → {PROD_USER}@{PROD_HOST}:{PROD_ASSETS_PATH}/cdn/");
    let remote = format!("{PROD_USER}@{PROD_HOST}");
    process::run(
        "ssh",
        &[
            &remote,
            &format!(
                "mkdir -p {PROD_ASSETS_PATH}/cdn/leaders {PROD_ASSETS_PATH}/cdn/ui {PROD_ASSETS_PATH}/cdn/avatars"
            ),
        ],
        None,
    )?;
    let src = format!("{}/", paths.assets_cdn.display());
    let dst = format!("{remote}:{PROD_ASSETS_PATH}/cdn/");
    process::run(
        "rsync",
        &[
            "-avz",
            "--chmod=Du=rwx,Dgo=rx,Fu=rw,Fgo=r",
            &src,
            &dst,
        ],
        None,
    )?;
    process::run(
        "ssh",
        &[&remote, &format!("chmod -R a+rX {PROD_ASSETS_PATH}/cdn")],
        None,
    )?;
    verify_prod_cdn()?;
    println!("✅ CDN pipeline OK");
    Ok(())
}

pub fn start_background(paths: &Paths) -> std::thread::JoinHandle<Result<()>> {
    let paths = paths.clone();
    std::thread::spawn(move || sync_to_prod(&paths))
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
    let cwebp = tools::cwebp()?;
    for (src, w, h, out) in [
        ("loader_empty.webp", "1032", "256", "loader_empty.webp"),
        ("loader_full.webp", "1032", "256", "loader_full.webp"),
        ("sow-splash-mobile.webp", "720", "1280", "sow-splash-mobile.webp"),
    ] {
        process::run(
            &cwebp,
            &[
                "-q",
                "82",
                "-resize",
                w,
                h,
                &ui_src.join(src).to_string_lossy(),
                "-o",
                &ui_cdn.join(out).to_string_lossy(),
            ],
            None,
        )?;
    }
    fs::copy(
        ui_src.join("sow-splash-desktop.webp"),
        ui_cdn.join("sow-splash-desktop.webp"),
    )?;
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

fn verify_prod_cdn() -> Result<()> {
    println!("==> Verifying prod CDN...");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let urls = [
        "https://shadowsofwar.io/assets/cdn/leaders/caesar_desktop.webp".to_string(),
        "https://shadowsofwar.io/assets/cdn/ui/loader_empty.webp".to_string(),
        "https://shadowsofwar.io/assets/cdn/avatars/caesar.webp".to_string(),
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
