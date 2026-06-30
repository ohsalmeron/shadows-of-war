use crate::paths::Paths;
use anyhow::{bail, Result};
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Router,
};
use std::collections::HashMap;
use std::path::PathBuf;
use tower_http::services::ServeDir;

#[derive(Clone)]
struct EditorState {
    root: PathBuf,
}

/// POST /__save?file=boudica.json — write the request body into `assets/campaign/<file>`.
/// Filename is restricted to a bare `*.json` (no path separators / traversal).
async fn save_campaign(
    State(st): State<EditorState>,
    Query(q): Query<HashMap<String, String>>,
    body: Bytes,
) -> impl IntoResponse {
    let file = q.get("file").cloned().unwrap_or_default();
    if file.is_empty() || file.contains('/') || file.contains('\\') || file.contains("..")
        || !file.ends_with(".json")
    {
        return (StatusCode::BAD_REQUEST, "bad file name").into_response();
    }
    let dir = st.root.join("assets/campaign");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("mkdir: {e}")).into_response();
    }
    let path = dir.join(&file);
    match std::fs::write(&path, &body) {
        Ok(_) => {
            println!("  ✎ saved {}", path.display());
            (StatusCode::OK, format!("saved {file}")).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {e}")).into_response(),
    }
}

/// POST /__launch — run the native client with the current data (cargo run --bin client).
/// Detached so the editor keeps serving; for a data-only change there's no recompile.
async fn launch_game(State(st): State<EditorState>) -> impl IntoResponse {
    println!("  ▶ launching game (cargo run --bin client)…");
    match std::process::Command::new("cargo")
        .args(["run", "--bin", "client", "--"])
        .current_dir(&st.root)
        .env("VERBOSE", "1")
        .spawn()
    {
        Ok(_) => (StatusCode::OK, "launched").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("spawn: {e}")).into_response(),
    }
}

/// POST /__save?file=demo.json — write the request body into `assets/ui/<file>`. Same
/// traversal-safe contract as `save_campaign`, scoped to `assets/ui/` for the scene editor.
async fn save_ui_scene(
    State(st): State<EditorState>,
    Query(q): Query<HashMap<String, String>>,
    body: Bytes,
) -> impl IntoResponse {
    let file = q.get("file").cloned().unwrap_or_default();
    if file.is_empty() || file.contains('/') || file.contains('\\') || file.contains("..")
        || !file.ends_with(".json")
    {
        return (StatusCode::BAD_REQUEST, "bad file name").into_response();
    }
    let dir = st.root.join("assets/ui");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("mkdir: {e}")).into_response();
    }
    let path = dir.join(&file);
    match std::fs::write(&path, &body) {
        Ok(_) => {
            println!("  ✎ saved {}", path.display());
            (StatusCode::OK, format!("saved {file}")).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {e}")).into_response(),
    }
}

/// POST /__export?scene=demo[&launch=1] — codegen `assets/ui/<scene>.json` into
/// `sow-client/src/ui_scene/generated/<scene>.rs` (see `crate::ui_gen`). With `&launch=1`, also
/// `cargo run --bin client` with `$SOW_UI_SCENE=<scene>` set, so "Export & Run" shows the 1:1
/// result in the real client — the compile step the fast browser loop intentionally defers.
async fn export_ui_scene(
    State(st): State<EditorState>,
    Query(q): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let Some(scene) = q.get("scene").cloned() else {
        return (StatusCode::BAD_REQUEST, "missing ?scene=").into_response();
    };
    let out = match crate::ui_gen::export_scene(&st.root, &scene) {
        Ok(path) => path,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("export failed: {e:#}")).into_response()
        }
    };
    println!("  ⚙ exported {}", out.display());

    if q.get("launch").map(|v| v == "1").unwrap_or(false) {
        println!("  ▶ launching preview (cargo run --bin client, SOW_UI_SCENE={scene})…");
        if let Err(e) = std::process::Command::new("cargo")
            .args(["run", "--bin", "client", "--"])
            .current_dir(&st.root)
            .env("VERBOSE", "1")
            .env("SOW_UI_SCENE", &scene)
            .spawn()
        {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("spawn: {e}")).into_response();
        }
    }
    (StatusCode::OK, format!("exported {}", out.display())).into_response()
}

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
    println!("\n  ┌─ Campaign editor ────────────────────────────────────");
    println!("  │  open:  {url}");
    println!("  │  Export writes assets/campaign/ directly; tick “launch”");
    println!("  │  to run the game on export. Ctrl-C to stop.");
    println!("  └──────────────────────────────────────────────────────\n");

    let root = paths.root.clone();
    let state = EditorState { root: root.clone() };
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let app = Router::new()
            .route("/__save", post(save_campaign))
            .route("/__launch", post(launch_game))
            .with_state(state)
            .fallback_service(ServeDir::new(&root));
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

/// Serve the UI Scene Editor backend: `/__save` writes `assets/ui/<scene>.json` (the source of
/// truth), `/__export` codegens it into `sow-client/src/ui_scene/generated/<scene>.rs` and
/// optionally launches a preview. The browser tool itself (`tools/ui-editor/`) is a later
/// milestone — these endpoints are stable now so it has a backend to target, and can be
/// exercised directly (e.g. via curl) in the meantime. `./sow ui`.
pub fn serve_ui_editor(paths: &Paths, port: u16) -> Result<()> {
    kill_port_holders(port);
    let bind = format!("127.0.0.1:{port}");
    println!("\n  ┌─ UI scene editor backend ──────────────────────────────");
    println!("  │  POST /__save?file=<scene>.json   → writes assets/ui/<scene>.json");
    println!("  │  POST /__export?scene=<scene>     → codegen (+ &launch=1 to preview)");
    println!("  │  serving repo root at http://{bind}/  Ctrl-C to stop.");
    println!("  └──────────────────────────────────────────────────────\n");

    let root = paths.root.clone();
    let state = EditorState { root: root.clone() };
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let app = Router::new()
            .route("/__save", post(save_ui_scene))
            .route("/__export", post(export_ui_scene))
            .with_state(state)
            .fallback_service(ServeDir::new(&root));
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
