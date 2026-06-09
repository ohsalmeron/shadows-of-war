use crate::config::DeployConfig;
use crate::gcp::GcpConfig;
use anyhow::{Context, Result};

pub fn resolve_remote_maps(gcp: &GcpConfig, unit: &str, env_var: &str, fallback: &str) -> String {
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

pub fn resolve_remote_workdir(gcp: &GcpConfig, unit: &str, env_var: &str, fallback: &str) -> String {
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
