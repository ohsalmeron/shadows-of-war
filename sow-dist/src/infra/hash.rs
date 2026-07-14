#![allow(dead_code, unused_imports, unused_variables)]
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
    "sow-data",
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

pub(crate) fn build_server_binaries(paths: &Paths) -> Result<(PathBuf, PathBuf, PathBuf, PathBuf)> {
    const GNU: &str = "x86_64-unknown-linux-gnu";
    process::wait_for_cargo_unlock(&paths.cargo_target);
    // Every shipped binary is named explicitly: `--bin` flags filter targets across ALL
    // selected packages, so a partial list would silently skip the rest and rsync would
    // push stale binaries. sow-database needs sow-data's `server` feature (wasm-clean lib).
    println!("==> cargo build --release --bin sow-server --bin sow-relay --bin sow-database --bin bot-manager ({GNU})");
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
            "sow-data",
            "-p",
            "sow-tools",
            "--features",
            "sow-data/server",
            "--bin",
            "sow-server",
            "--bin",
            "sow-relay",
            "--bin",
            "sow-database",
            "--bin",
            "bot-manager",
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
        dir.join("bot-manager"),
    ))
}

pub fn build_freebsd_server_binaries(paths: &Paths) -> Result<(PathBuf, PathBuf, PathBuf, PathBuf)> {
    println!("==> Syncing codebase to local FreeBSD VM via rsync...");
    
    // Sync the local repository files to the VM
    let local_root = paths.root.to_str().unwrap();
    let rsync_status = std::process::Command::new("rsync")
        .args(&[
            "-az",
            "-e",
            "ssh -i /home/bizkit/.ssh/id_ed25519 -p 2222 -o StrictHostKeyChecking=no",
            "--exclude", "target",
            "--exclude", ".git",
            "--exclude", "dist",
            "--exclude", "sow-web/node_modules",
            &format!("{}/", local_root),
            "root@127.0.0.1:/root/build/",
        ])
        .status()?;

    if !rsync_status.success() {
        anyhow::bail!("Failed to sync codebase to local FreeBSD VM");
    }

    println!("==> Running release build on local FreeBSD VM (cargo build --release)...");
    let ssh_status = std::process::Command::new("ssh")
        .args(&[
            "-i", "/home/bizkit/.ssh/id_ed25519",
            "-p", "2222",
            "-o", "StrictHostKeyChecking=no",
            "root@127.0.0.1",
            "cd /root/build && /root/.cargo/bin/cargo build --release -p sow-server -p sow-relay -p sow-data -p sow-tools --features sow-data/server --bin sow-server --bin sow-relay --bin sow-database --bin bot-manager",
        ])
        .status()?;

    if !ssh_status.success() {
        anyhow::bail!("Cargo build failed inside local FreeBSD VM");
    }

    let freebsd_target_dir = paths.cargo_target.join("x86_64-unknown-freebsd/release");
    std::fs::create_dir_all(&freebsd_target_dir)?;

    println!("==> Fetching compiled FreeBSD binaries back to host target directory: {:?}", freebsd_target_dir);
    for name in &["sow-server", "sow-relay", "sow-database", "bot-manager"] {
        let scp_status = std::process::Command::new("scp")
            .args(&[
                "-i", "/home/bizkit/.ssh/id_ed25519",
                "-P", "2222",
                "-o", "StrictHostKeyChecking=no",
                &format!("root@127.0.0.1:/root/build/target/release/{}", name),
                freebsd_target_dir.join(name).to_str().unwrap(),
            ])
            .status()?;
        if !scp_status.success() {
            anyhow::bail!("Failed to fetch compiled binary '{}' from FreeBSD VM", name);
        }
    }

    println!("✅ Remote FreeBSD compilation complete and binaries retrieved!");
    Ok((
        freebsd_target_dir.join("sow-server"),
        freebsd_target_dir.join("sow-relay"),
        freebsd_target_dir.join("sow-database"),
        freebsd_target_dir.join("bot-manager"),
    ))
}

pub(crate) fn rsync_server_binaries(
    gcp: &GcpConfig,
    data_dir: &str,
    server: &Path,
    relay: &Path,
    database: &Path,
    bot_manager: &Path,
) -> Result<()> {
    // scp writes into the destination inode and fails with ETXTBSY while that
    // binary is executing — relays keep running across deploys by design. Upload
    // to a temp name and rename: running processes keep their old inode, the
    // path serves the new binary from the next spawn/restart.
    for (local, name) in [
        (server, "sow-server"),
        (relay, "sow-relay"),
        (database, "sow-database"),
        (bot_manager, "bot-manager"),
    ] {
        let dest = format!("{data_dir}/{name}");
        gcp.sync_file(local, &format!("{dest}.new"))?;
        gcp.run_remote(&format!("chmod +x {dest}.new && mv -f {dest}.new {dest}"))?;
    }
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
