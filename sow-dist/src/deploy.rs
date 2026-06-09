use crate::config::DeployConfig;
use crate::gcp::GcpConfig;
use crate::infra;
use crate::paths::{
    remote_data_prod, remote_data_ptr, remote_maps_prod, remote_maps_ptr, Paths,
};
use anyhow::{Context, Result};

fn resolve_remote_maps(gcp: &GcpConfig, unit: &str, env_var: &str, fallback: &str) -> String {
    if let Ok(p) = std::env::var(env_var) {
        return p;
    }
    if let Ok(out) = gcp.remote_output(&format!(
        "systemctl show {unit} -p Environment --value 2>/dev/null || true"
    )) {
        for token in out.split_whitespace() {
            if let Some(v) = token.strip_prefix("SOW_MAPS_ROOT=") {
                return v.to_string();
            }
        }
    }
    fallback.to_string()
}

fn resolve_remote_workdir(gcp: &GcpConfig, unit: &str, env_var: &str, fallback: &str) -> String {
    if let Ok(p) = std::env::var(env_var) {
        return p;
    }
    if let Ok(out) = gcp.remote_output(&format!(
        "systemctl show {unit} -p WorkingDirectory --value 2>/dev/null || true"
    )) {
        let wd = out.trim();
        if !wd.is_empty() {
            return wd.to_string();
        }
    }
    fallback.to_string()
}

pub fn deploy_prod(paths: &Paths, cfg: &DeployConfig, version: &str) -> Result<()> {
    let gcp = cfg.gcp();
    let remote_home = gcp.remote_home(&paths.remote_home_cache())?;
    println!(
        "==> Deploying prod content → {} + marketing → {}",
        cfg.play_domain(),
        cfg.site_domain()
    );
    let maps_dir = resolve_remote_maps(
        &gcp,
        "sow-server",
        "SOW_REMOTE_MAPS_PROD",
        &remote_maps_prod(&remote_home),
    );
    let cache = paths.dist_root();
    let web_play = cfg.web_root_play();
    let web_main = cfg.web_root_main();
    std::thread::scope(|s| -> Result<()> {
        let gcp_a = gcp.clone();
        let gcp_b = gcp.clone();
        let gcp_f = gcp.clone();
        let dist = paths.dist_play.display().to_string();
        let site = paths.site_web.display().to_string();
        let maps = paths.assets_maps.display().to_string();
        let cache_a = cache.clone();
        let cache_b = cache.clone();
        let cache_f = cache.clone();
        let a = s.spawn(move || {
            gcp_a.rsync_dir_with_opts(
                &cache_a,
                &dist,
                &web_play,
                &["-avzL", "--delete", "--exclude=*.bin"],
            )
        });
        let b = s.spawn(move || {
            gcp_b.rsync_dir_with_opts(&cache_b, &site, &web_main, &["-avz"])
        });
        let f = s.spawn(move || {
            gcp_f.rsync_dir_with_opts(
                &cache_f,
                &maps,
                maps_dir.trim_end_matches('/'),
                &[
                    "-avz",
                    "--exclude=map.bin",
                    "--exclude=mini_map.bin",
                    "--exclude=manifest.json",
                    "--exclude=maps.json",
                ],
            )
        });
        a.join().unwrap()?;
        b.join().unwrap()?;
        f.join().unwrap()?;
        Ok(())
    })?;
    gcp.run_remote("sudo restorecon -R /var/www")?;
    verify_play_host(&cfg.play_url())?;
    verify_marketing_embed(&format!("{}/", cfg.site_url()))?;
    verify_sitemap(&cfg, &cfg.sitemap_url())?;
    infra::deploy_server_release(
        paths,
        &gcp,
        "sow-server",
        &resolve_remote_workdir(
            &gcp,
            "sow-server",
            "SOW_REMOTE_DATA_PROD",
            &remote_data_prod(&remote_home),
        ),
        version,
        &cfg.maps_url(&cfg.site_origin),
        &cfg.ws_url(&cfg.site_origin),
    )?;
    Ok(())
}

pub fn deploy_ptr(paths: &Paths, cfg: &DeployConfig, version: &str) -> Result<()> {
    let gcp = cfg.gcp();
    let remote_home = gcp.remote_home(&paths.remote_home_cache())?;
    println!("==> Deploying ptr content → {}", cfg.ptr_domain());
    let maps_dir = resolve_remote_maps(
        &gcp,
        "sow-server-ptr",
        "SOW_REMOTE_MAPS_PTR",
        &remote_maps_ptr(&remote_home),
    );
    let cache = paths.dist_root();
    let dist = paths.dist_ptr.display().to_string();
    let maps = paths.assets_maps.display().to_string();
    let web_ptr = cfg.web_root_ptr();
    std::thread::scope(|s| -> Result<()> {
        let gcp_a = gcp.clone();
        let gcp_d = gcp.clone();
        let cache_a = cache.clone();
        let cache_d = cache.clone();
        let a = s.spawn(move || {
            gcp_a.rsync_dir_with_opts(
                &cache_a,
                &dist,
                &web_ptr,
                &["-avzL", "--delete", "--exclude=*.bin"],
            )
        });
        let d = s.spawn(move || {
            gcp_d.rsync_dir_with_opts(
                &cache_d,
                &maps,
                maps_dir.trim_end_matches('/'),
                &[
                    "-avz",
                    "--exclude=map.bin",
                    "--exclude=mini_map.bin",
                    "--exclude=manifest.json",
                    "--exclude=maps.json",
                ],
            )
        });
        a.join().unwrap()?;
        d.join().unwrap()?;
        Ok(())
    })?;
    verify_play_host(&cfg.ptr_url())?;
    infra::deploy_server_release(
        paths,
        &gcp,
        "sow-server-ptr",
        &resolve_remote_workdir(
            &gcp,
            "sow-server-ptr",
            "SOW_REMOTE_DATA_PTR",
            &remote_data_ptr(&remote_home),
        ),
        version,
        &cfg.maps_url(&cfg.ptr_origin),
        &cfg.ws_url(&cfg.ptr_origin),
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

pub fn verify_sitemap(cfg: &DeployConfig, sitemap_url: &str) -> Result<()> {
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
    let site = cfg.site_url();
    for path in ["/", "/privacy", "/terms"] {
        let loc = format!("{site}{path}");
        if !body.contains(&loc) {
            anyhow::bail!("sitemap.xml missing {loc}");
        }
    }
    println!("✅ Sitemap OK ({url_count} URLs, Content-Type: {ctype})");
    Ok(())
}
