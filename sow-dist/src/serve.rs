use crate::paths::Paths;
use crate::process;
use anyhow::{bail, Context, Result};

pub fn serve_site_dev(paths: &Paths, port: u16) -> Result<()> {
    let www = &paths.dist_site_dev_www;
    if !www.join("index.html").is_file() {
        bail!(
            "missing {} — run: cargo run -p sow-dist -- localsite",
            www.display()
        );
    }
    let bind = format!("127.0.0.1:{port}");
    println!("==> Serving {} at http://{bind}/", www.display());
    println!("    Embedded game: http://{bind}/game/");
    process::run(
        "python3",
        &[
            "-m",
            "http.server",
            &port.to_string(),
            "--bind",
            "127.0.0.1",
            "--directory",
            &www.to_string_lossy(),
        ],
        None,
    )
    .context("python3 -m http.server (install Python 3 if missing)")?;
    Ok(())
}
