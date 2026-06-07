use crate::infra;
use crate::paths::{deploy_host, deploy_user, remote_maps_prod, remote_maps_ptr, Paths};
use crate::process;
use anyhow::{Context, Result};

/// Remote maps dir: env override, else read `SOW_MAPS_ROOT` from the live systemd unit.
fn resolve_remote_maps(remote: &str, unit: &str, env_var: &str, fallback: &str) -> String {
    if let Ok(p) = std::env::var(env_var) {
        return p;
    }
    if let Ok(out) = process::output(
        "ssh",
        &[
            remote,
            &format!("systemctl show {unit} -p Environment --value 2>/dev/null || true"),
        ],
    ) {
        for token in out.split_whitespace() {
            if let Some(v) = token.strip_prefix("SOW_MAPS_ROOT=") {
                return v.to_string();
            }
        }
    }
    fallback.to_string()
}

pub fn deploy_prod(paths: &Paths) -> Result<()> {
    let remote = format!("{}@{}", deploy_user(), deploy_host());
    println!("==> Deploying prod content → play.shadowsofwar.io + marketing → shadowsofwar.io");
    let dist = format!("{}/", paths.dist_play.display());
    let play_html = format!("{remote}:/var/www/play.shadowsofwar.io/html/");
    let main_html = format!("{remote}:/var/www/shadowsofwar.io/html/");
    let site = format!("{}/", paths.site_web.display());
    let maps = format!("{}/", paths.assets_maps.display());
    let maps_dir = resolve_remote_maps(
        &remote,
        "sow-server",
        "SOW_REMOTE_MAPS_PROD",
        &remote_maps_prod(),
    );
    let maps_remote = format!("{remote}:{}/", maps_dir.trim_end_matches('/'));
    std::thread::scope(|s| -> Result<()> {
        let a = s.spawn(|| {
            process::run(
                "rsync",
                &["-avzL", "--delete", "--exclude=*.bin", &dist, &play_html],
                None,
            )
        });
        let b = s.spawn(|| process::run("rsync", &["-avz", &site, &main_html], None));
        let f = s.spawn(|| {
            process::run(
                "rsync",
                &[
                    "-avz",
                    "--exclude=map.bin",
                    "--exclude=mini_map.bin",
                    "--exclude=manifest.json",
                    "--exclude=maps.json",
                    &maps,
                    &maps_remote,
                ],
                None,
            )
        });
        a.join().unwrap()?;
        b.join().unwrap()?;
        f.join().unwrap()?;
        Ok(())
    })?;
    verify_play_host("https://play.shadowsofwar.io/")?;
    verify_marketing_embed("https://shadowsofwar.io/")?;
    verify_sitemap("https://shadowsofwar.io/sitemap.xml")?;
    infra::verify_server_health(
        &remote,
        "https://shadowsofwar.io/maps/catalog.bin",
        "https://shadowsofwar.io/ws/",
        "sow-server",
    )?;
    Ok(())
}

pub fn deploy_ptr(paths: &Paths) -> Result<()> {
    let remote = format!("{}@{}", deploy_user(), deploy_host());
    println!("==> Deploying ptr content → ptr.shadowsofwar.io");
    let dist = format!("{}/", paths.dist_ptr.display());
    let html = format!("{remote}:/var/www/ptr.shadowsofwar.io/html/");
    let maps = format!("{}/", paths.assets_maps.display());
    let maps_dir = resolve_remote_maps(
        &remote,
        "sow-server-ptr",
        "SOW_REMOTE_MAPS_PTR",
        &remote_maps_ptr(),
    );
    let maps_remote = format!("{remote}:{}/", maps_dir.trim_end_matches('/'));
    std::thread::scope(|s| -> Result<()> {
        let a = s.spawn(|| process::run("rsync", &["-avzL", "--delete", "--exclude=*.bin", &dist, &html], None));
        let d = s.spawn(|| {
            process::run(
                "rsync",
                &[
                    "-avz",
                    "--exclude=map.bin",
                    "--exclude=mini_map.bin",
                    "--exclude=manifest.json",
                    "--exclude=maps.json",
                    &maps,
                    &maps_remote,
                ],
                None,
            )
        });
        a.join().unwrap()?;
        d.join().unwrap()?;
        Ok(())
    })?;
    verify_play_host("https://ptr.shadowsofwar.io/")?;
    infra::verify_server_health(
        &remote,
        "https://ptr.shadowsofwar.io/maps/catalog.bin",
        "https://ptr.shadowsofwar.io/ws/",
        "sow-server-ptr",
    )?;
    Ok(())
}

pub fn verify_play_host(play_url: &str) -> Result<()> {
    println!("==> Verifying {play_url}");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let manifest_url = format!("{play_url}game-manifest.json");
    let manifest = client.get(&manifest_url).send().context("manifest")?;
    if !manifest.status().is_success() {
        anyhow::bail!("game-manifest.json failed");
    }
    let body = manifest.text()?;
    if !body.contains("sow_client") {
        anyhow::bail!("invalid game-manifest.json");
    }
    let html = client.get(play_url).send().context("index")?.text()?;
    if !html.contains("web-loader") || !html.contains("hideWebLoader") {
        anyhow::bail!("index.html missing loader");
    }
    if !html.contains("sow_client") && !body.contains("_bg.wasm") {
        anyhow::bail!("missing sow_client bundle reference");
    }
    println!("✅ Play host OK");
    Ok(())
}

pub fn verify_marketing_embed(home_url: &str) -> Result<()> {
    println!("==> Verifying marketing embed at {home_url}");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let html = client.get(home_url).send().context("index")?.text()?;
    if !html.contains("sow-game-stage") || !html.contains("game-embed.js") {
        anyhow::bail!("marketing index missing iframe embed");
    }
    if !html.contains("iframe") {
        anyhow::bail!("marketing index missing iframe player");
    }
    if html.contains("sow_client_") {
        anyhow::bail!("marketing index must not reference WASM bundle before Play click");
    }
    let embed = client
        .get(format!("{home_url}game-embed.js"))
        .send()
        .context("game-embed.js")?;
    if !embed.status().is_success() {
        anyhow::bail!("game-embed.js missing on marketing host");
    }
    println!("✅ Marketing embed OK");
    Ok(())
}

pub fn verify_sitemap(sitemap_url: &str) -> Result<()> {
    println!("==> Verifying sitemap at {sitemap_url}");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let res = client
        .get(sitemap_url)
        .header("User-Agent", "Googlebot")
        .send()
        .context("sitemap fetch")?;
    if !res.status().is_success() {
        anyhow::bail!("sitemap.xml returned {}", res.status());
    }
    let ctype = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !ctype.contains("xml") {
        anyhow::bail!("sitemap Content-Type must be xml, got: {ctype}");
    }
    let body = res.text()?;
    if !body.contains("<urlset") || !body.contains("</urlset>") {
        anyhow::bail!("sitemap.xml is not a valid urlset document");
    }
    let url_count = body.matches("<loc>").count();
    if url_count < 3 {
        anyhow::bail!("sitemap.xml expected at least 3 URLs, found {url_count}");
    }
    for path in ["/", "/privacy", "/terms"] {
        let loc = format!("https://shadowsofwar.io{path}");
        if !body.contains(&loc) {
            anyhow::bail!("sitemap.xml missing {loc}");
        }
    }
    println!("✅ Sitemap OK ({url_count} URLs, Content-Type: {ctype})");
    Ok(())
}
