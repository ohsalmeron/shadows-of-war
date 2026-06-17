use crate::paths::Paths;
use anyhow::{bail, Result};
use axum::Router;
use tower_http::services::ServeDir;

pub fn serve_site_dev(paths: &Paths, port: u16) -> Result<()> {
    let www = &paths.dist_site_dev_www;
    if !www.join("index.html").is_file() {
        bail!("missing {} — run: ./sow local", www.display());
    }
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
