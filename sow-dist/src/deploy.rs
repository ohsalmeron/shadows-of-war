use crate::config::DeployConfig;
use anyhow::{Context, Result};

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
    let html = client.get(&format!("{play_url}play/")).send().context("index")?.text()?;
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
    println!("==> Verifying main website at {home_url}");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let html = client.get(home_url).send().context("index")?.text()?;
    if !html.contains("href=\"/play/\"") {
        anyhow::bail!("website index missing link to /play/");
    }
    if html.contains("sow_client_") {
        anyhow::bail!("website index must not load WASM bundle directly on homepage");
    }
    println!("✅ Main website OK");
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
