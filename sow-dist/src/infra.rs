use crate::paths::{deploy_user, Paths};
use crate::process;
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

const SERVER_CRATES: &[&str] = &["sow-server", "sow-relay", "sow-core", "sow-net"];

pub fn deploy_infra(paths: &Paths, host: &str) -> Result<()> {
    let remote = format!("{}@{host}", deploy_user());
    if remote_is_nixos(&remote)? {
        install_nixos_rebuild(&remote, paths)?;
    } else {
        install_nixos_anywhere(&remote, paths)?;
    }
    write_infra_hash(paths)?;
    println!("✅ NixOS infra applied on {host}");
    Ok(())
}

fn remote_is_nixos(remote: &str) -> Result<bool> {
    let out = process::output(
        "ssh",
        &[
            remote,
            "test -f /etc/NIXOS && echo yes || echo no",
        ],
    )?;
    Ok(out.trim() == "yes")
}

fn install_nixos_rebuild(remote: &str, paths: &Paths) -> Result<()> {
    println!("==> nixos-rebuild switch --flake .#vps --target-host {remote}");
    process::run(
        "nix",
        &[
            "run",
            "--inputs-from",
            ".",
            "nixpkgs#nixos-rebuild",
            "--",
            "switch",
            "--flake",
            ".#vps",
            "--target-host",
            remote,
            "--build-host",
            remote,
        ],
        Some(&paths.root),
    )?;
    Ok(())
}

fn install_nixos_anywhere(remote: &str, paths: &Paths) -> Result<()> {
    println!("==> nixos-anywhere --flake .#vps-install --target-host {remote}");
    println!("    (replaces Debian — disk repartitioned, ~5–15 min downtime)");
    process::run(
        "nix",
        &[
            "run",
            ".#nixos-anywhere",
            "--",
            "--flake",
            ".#vps-install",
            "--target-host",
            remote,
        ],
        Some(&paths.root),
    )?;
    Ok(())
}

fn write_infra_hash(paths: &Paths) -> Result<()> {
    fs::write(paths.infra_hash_cache(), hash_server_inputs(paths)?)?;
    Ok(())
}

fn hash_server_inputs(paths: &Paths) -> Result<String> {
    let mut h = Sha256::new();
    for name in SERVER_CRATES {
        hash_dir(&mut h, &paths.root.join(name))?;
    }
    hash_file(&mut h, &paths.root.join("Cargo.lock"))?;
    hash_file(&mut h, &paths.root.join("flake.lock"))?;
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

pub fn verify_server_health(remote: &str, maps_url: &str, ws_url: &str, unit: &str) -> Result<()> {
    println!("==> Verifying server ({unit}) on VPS");
    let active = process::output("ssh", &[remote, &format!("systemctl is-active {unit}")])?;
    if active.trim() != "active" {
        let logs = process::output(
            "ssh",
            &[remote, &format!("journalctl -u {unit} -n 30 --no-pager")],
        )
        .unwrap_or_else(|_| String::from("(journalctl unavailable)"));
        eprintln!("{logs}");
        bail!("{unit} is not active (got: {})", active.trim());
    }
    println!("✅ {unit} active");

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
    if cors != "*" && !cors.contains("shadowsofwar.io") {
        bail!("maps API missing CORS header (got: {cors:?})");
    }
    println!("✅ Maps API OK");

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
