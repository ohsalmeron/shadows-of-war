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
