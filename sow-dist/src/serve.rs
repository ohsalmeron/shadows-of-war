use crate::paths::Paths;
use anyhow::{bail, Result};
use axum::Router;
use tower_http::services::ServeDir;

fn kill_port_holders(port: u16) {
    let port_str = port.to_string();
    if let Ok(out) = std::process::Command::new("lsof")
        .args(["-t", "-i", &format!("tcp:{port_str}")])
        .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            let pid = line.trim();
            if !pid.is_empty() {
                println!("==> Port {port} is in use. Killing process {pid}...");
                let _ = std::process::Command::new("kill")
                    .args(["-9", pid])
                    .status();
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
    }
}

/// Serve the repo root so the standalone campaign roster editor can read the boudica terrain
/// (`tools/campaign-editor/boudica.bin`) and the live roster (`assets/campaign/boudica.json`).
/// Decodes the map from the local game cache on first run. `./sow m`.
pub fn serve_campaign_editor(paths: &Paths, port: u16) -> Result<()> {
    ensure_editor_map(paths);
    kill_port_holders(port);
    let bind = format!("127.0.0.1:{port}");
    let url = format!("http://{bind}/tools/campaign-editor/");
    println!("\n  ┌─ Campaign roster editor ─────────────────────────────");
    println!("  │  open:  {url}");
    println!("  │  drag tribes → Download boudica.json → assets/campaign/");
    println!("  │  then relaunch the game (no recompile). Ctrl-C to stop.");
    println!("  └──────────────────────────────────────────────────────\n");

    let root = paths.root.clone();
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let app = Router::new().fallback_service(ServeDir::new(&root));
        match tokio::net::TcpListener::bind(&bind).await {
            Ok(listener) => axum::serve(listener, app)
                .await
                .map_err(|e| anyhow::anyhow!("Server error: {e}")),
            Err(e) => anyhow::bail!(
                "Failed to bind to {bind}. Is the port already in use?\nDetailed error: {e}"
            ),
        }
    })?;
    Ok(())
}

/// Decode the cached boudica map (brotli) into the editor dir if not already there. The `.bin` is
/// gitignored CDN map data; the editor needs the raw terrain to draw land/water under the tribes.
fn ensure_editor_map(paths: &Paths) {
    let dst = paths.root.join("tools/campaign-editor/boudica.bin");
    if dst.is_file() {
        return;
    }
    let data_dir = std::env::var("SOW_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join(".local/share/shadows-of-war")
        });
    let src = data_dir.join("maps/boudica.bin.br");
    let Ok(compressed) = std::fs::read(&src) else {
        println!(
            "  note: no cached map at {} — play the boudica tutorial once so it caches, then re-run `./sow m`.",
            src.display()
        );
        return;
    };
    use std::io::Read;
    let mut out = Vec::new();
    if brotli::Decompressor::new(&compressed[..], 4096)
        .read_to_end(&mut out)
        .is_ok()
    {
        let _ = dst.parent().map(std::fs::create_dir_all);
        if std::fs::write(&dst, &out).is_ok() {
            println!("  decoded map → {}", dst.display());
        }
    }
}

pub fn serve_site_dev(paths: &Paths, port: u16) -> Result<()> {
    let www = &paths.dist_site_dev_www;
    if !www.join("index.html").is_file() {
        bail!("missing {} — run: ./sow local", www.display());
    }
    kill_port_holders(port);
    let bind = format!("127.0.0.1:{port}");
    println!("==> Serving {} at http://{bind}/", www.display());
    println!("    Embedded game: http://{bind}/game/");

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let app = Router::new().fallback_service(ServeDir::new(www));
        let listener = tokio::net::TcpListener::bind(&bind).await;
        match listener {
            Ok(listener) => axum::serve(listener, app)
                .await
                .map_err(|e| anyhow::anyhow!("Server error: {e}")),
            Err(e) => anyhow::bail!(
                "Failed to bind to {bind}. Is the port already in use?\nDetailed error: {e}"
            ),
        }
    })?;
    Ok(())
}
