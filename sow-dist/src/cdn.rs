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

/// Full CDN pipeline: prepare repo tree, rsync to marketing host, verify HTTP.
pub fn sync_to_prod(paths: &Paths) -> Result<()> {
    prepare_boot_ui(paths)?;
    check_leader_portraits(&paths.assets_cdn.join("leaders"))?;
    println!("==> Syncing CDN → {PROD_USER}@{PROD_HOST}:{PROD_ASSETS_PATH}/cdn/");
    let remote = format!("{PROD_USER}@{PROD_HOST}");
    process::run(
        "ssh",
        &[
            &remote,
            &format!("mkdir -p {PROD_ASSETS_PATH}/cdn/leaders {PROD_ASSETS_PATH}/cdn/ui"),
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
    sync_marketing_nginx(paths)?;
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

fn sync_marketing_nginx(paths: &Paths) -> Result<()> {
    let local = paths.deploy_nginx.join("shadowsofwar.io.conf");
    let remote = format!("{PROD_USER}@{PROD_HOST}");
    let local_s = local.to_string_lossy();
    let hash = process::output("md5sum", &[local_s.as_ref()])?;
    let hash = hash.split_whitespace().next().context("md5")?;
    process::run(
        "scp",
        &[local_s.as_ref(), &format!("{remote}:/tmp/sow-nginx.conf")],
        None,
    )?;
    let remote_script = format!(
        "set -euo pipefail; export PATH=/usr/sbin:/usr/bin:/sbin:/bin:$PATH; \
         NGINX_SITE=/etc/nginx/sites-available/shadowsofwar.io; LOCAL_HASH={hash}; \
         changed=0; remote_hash=; \
         [[ -f \"$NGINX_SITE\" ]] && remote_hash=$(md5sum \"$NGINX_SITE\" | awk '{{print $1}}'); \
         if [[ \"$remote_hash\" != \"$LOCAL_HASH\" ]]; then \
           echo '==> Updating nginx...'; sudo cp /tmp/sow-nginx.conf \"$NGINX_SITE\"; \
           sudo ln -sf \"$NGINX_SITE\" /etc/nginx/sites-enabled/shadowsofwar.io; \
           sudo nginx -t && sudo systemctl reload nginx; echo '✅ Nginx reloaded.'; \
         else echo '✅ Nginx config unchanged.'; fi"
    );
    process::run("ssh", &[&remote, &remote_script], None)?;
    Ok(())
}

fn verify_prod_cdn() -> Result<()> {
    println!("==> Verifying prod CDN...");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    for url in [
        "https://shadowsofwar.io/assets/cdn/leaders/caesar_desktop.webp",
        "https://shadowsofwar.io/assets/cdn/ui/loader_empty.webp",
        "https://shadowsofwar.io/assets/fonts/PressStart2P-Regular.ttf",
    ] {
        let resp = client.head(url).send().context(url)?;
        if !resp.status().is_success() {
            bail!("CDN verify failed: {url} → {}", resp.status());
        }
    }
    println!("✅ prod CDN assets OK");
    Ok(())
}
