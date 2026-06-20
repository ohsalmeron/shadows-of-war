use crate::gcp::GcpConfig;
use crate::paths::Paths;
use crate::process;
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const SERVER_CRATES: &[&str] = &[
    "sow-server",
    "sow-relay",
    "sow-database",
    "sow-core",
    "sow-net",
];

pub(crate) fn write_infra_hash(paths: &Paths) -> Result<()> {
    fs::write(paths.infra_hash_cache(), hash_server_inputs(paths)?)?;
    Ok(())
}

pub(crate) fn hash_server_inputs(paths: &Paths) -> Result<String> {
    let mut h = Sha256::new();
    for name in SERVER_CRATES {
        hash_dir(&mut h, &paths.root.join(name))?;
    }
    hash_file(&mut h, &paths.root.join("Cargo.lock"))?;
    Ok(format!("{:x}", h.finalize()))
}

fn hash_dir(h: &mut Sha256, dir: &Path) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            hash_dir(h, &path)?;
        } else if path.extension().is_some_and(|e| e == "rs" || e == "toml") {
            hash_file(h, &path)?;
        }
    }
    Ok(())
}

fn hash_file(h: &mut Sha256, path: &Path) -> Result<()> {
    if path.is_file() {
        h.update(fs::read(path)?);
    }
    Ok(())
}

pub(crate) fn read_cached_hash(paths: &Paths) -> String {
    fs::read_to_string(paths.infra_hash_cache())
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub(crate) fn build_server_binaries(paths: &Paths) -> Result<(PathBuf, PathBuf, PathBuf)> {
    const GNU: &str = "x86_64-unknown-linux-gnu";
    process::wait_for_cargo_unlock(&paths.cargo_target);
    println!("==> cargo build --release -p sow-server -p sow-relay -p sow-database ({GNU})");
    process::run(
        "cargo",
        &[
            "build",
            "--release",
            "-p",
            "sow-server",
            "-p",
            "sow-relay",
            "-p",
            "sow-database",
            "--target",
            GNU,
        ],
        Some(&paths.root),
    )?;
    let dir = paths.cargo_target.join(format!("{GNU}/release"));
    Ok((
        dir.join("sow-server"),
        dir.join("sow-relay"),
        dir.join("sow-database"),
    ))
}

pub(crate) fn rsync_server_binaries(
    gcp: &GcpConfig,
    data_dir: &str,
    server: &Path,
    relay: &Path,
    database: &Path,
) -> Result<()> {
    gcp.sync_file(server, &format!("{data_dir}/sow-server"))?;
    gcp.sync_file(relay, &format!("{data_dir}/sow-relay"))?;
    gcp.sync_file(database, &format!("{data_dir}/sow-database"))?;
    gcp.run_remote(&format!(
        "chmod +x {data_dir}/sow-server {data_dir}/sow-relay {data_dir}/sow-database"
    ))?;
    Ok(())
}

pub fn verify_server_health(
    gcp: &GcpConfig,
    maps_url: &str,
    ws_url: &str,
    db_url: &str,
    unit: &str,
    db_unit: &str,
) -> Result<()> {
    for u in &[unit, db_unit] {
        println!("==> Verifying service ({u}) on VPS");
        let active = gcp.remote_output(&format!("systemctl is-active {u}"))?;
        if active.trim() != "active" {
            let logs = gcp
                .remote_output(&format!("journalctl -u {u} -n 30 --no-pager"))
                .unwrap_or_else(|_| String::from("(journalctl unavailable)"));
            eprintln!("{logs}");
            bail!("{u} is not active (got: {})", active.trim());
        }
        println!("✅ {u} active");
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    println!("==> Verifying maps API {maps_url}");
    let maps = client.get(maps_url).send().context("maps fetch")?;
    if !maps.status().is_success() {
        bail!("maps API failed: {} → {}", maps_url, maps.status());
    }
    let cors = maps
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if cors != "*" {
        bail!("maps API missing CORS header (got: {cors:?})");
    }

    println!("==> Verifying database API {db_url}");
    let db_resp = client.get(db_url).send().context("db fetch")?;
    if !db_resp.status().is_success() {
        let status = db_resp.status();
        let body = db_resp.text().unwrap_or_default();
        bail!(
            "database API failed: {} → {} (body: {})",
            db_url,
            status,
            body
        );
    }
    println!("✅ Database API OK");

    println!("==> Verifying WebSocket {ws_url}");
    let ws_resp = client
        .get(ws_url)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .send()
        .context("ws probe")?;
    let status = ws_resp.status().as_u16();
    if status == 502 || status == 503 || status == 504 {
        bail!("WebSocket proxy failed: {ws_url} → HTTP {status}");
    }
    println!("✅ WebSocket proxy reachable (HTTP {status})");
    Ok(())
}

fn hash_deploy_dir(h: &mut Sha256, dir: &Path) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            hash_deploy_dir(h, &path)?;
        } else if path.is_file() {
            hash_file(h, &path)?;
        }
    }
    Ok(())
}

pub fn hash_deploy_templates(paths: &Paths) -> Result<String> {
    let mut h = Sha256::new();
    hash_deploy_dir(&mut h, &paths.deploy_dir())?;
    Ok(format!("{:x}", h.finalize()))
}
