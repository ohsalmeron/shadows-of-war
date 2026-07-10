use super::hash::{
    build_server_binaries, hash_server_inputs, read_cached_hash, rsync_server_binaries,
    write_infra_hash,
};
use crate::gcp::GcpConfig;
use crate::paths::Paths;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ServerArtifacts {
    pub server: PathBuf,
    pub relay: PathBuf,
    pub database: PathBuf,
    pub bot_manager: PathBuf,
    pub built: bool,
}

#[derive(Clone, Debug)]
pub struct ServerShipResult {
    pub shipped_binaries: bool,
    pub version_changed: bool,
}

pub fn local_server_binaries(paths: &Paths) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    const GNU: &str = "x86_64-unknown-linux-gnu";
    let dir = paths.cargo_target.join(format!("{GNU}/release"));
    (
        dir.join("sow-server"),
        dir.join("sow-relay"),
        dir.join("sow-database"),
        dir.join("bot-manager"),
    )
}

pub fn needs_local_server_build(paths: &Paths) -> Result<bool> {
    let current_hash = hash_server_inputs(paths)?;
    let hash_changed = read_cached_hash(paths) != current_hash;
    let (server, relay, database, bot_manager) = local_server_binaries(paths);
    Ok(hash_changed || !server.is_file() || !relay.is_file() || !database.is_file() || !bot_manager.is_file())
}

/// Phase 1: compile server binaries locally when crate inputs changed.
pub fn build_server_if_needed(paths: &Paths) -> Result<ServerArtifacts> {
    let (server, relay, database, bot_manager) = local_server_binaries(paths);
    if needs_local_server_build(paths)? {
        let (server, relay, database, bot_manager) = build_server_binaries(paths)?;
        Ok(ServerArtifacts {
            server,
            relay,
            database,
            bot_manager,
            built: true,
        })
    } else {
        println!("==> Server crates unchanged — skipping server build");
        Ok(ServerArtifacts {
            server,
            relay,
            database,
            bot_manager,
            built: false,
        })
    }
}

pub fn remote_binaries_missing(gcp: &GcpConfig, data_dir: &str) -> bool {
    gcp.remote_output(&format!(
        "test -x {data_dir}/sow-server && test -x {data_dir}/sow-relay && test -x {data_dir}/sow-database && test -x {data_dir}/bot-manager && echo ok"
    ))
    .map(|s| s.trim() != "ok")
    .unwrap_or(true)
}

/// Phase 3: sync server binaries and `.version` (no restart).
pub fn ship_server(
    paths: &Paths,
    gcp: &GcpConfig,
    data_dir: &str,
    artifacts: &ServerArtifacts,
    version: &str,
    unit: &str,
) -> Result<ServerShipResult> {
    let remote_missing = remote_binaries_missing(gcp, data_dir);
    let need_ship = artifacts.built || remote_missing;
    if need_ship {
        // No service stop here: binaries land via upload-to-temp + rename, so
        // running processes (incl. relays, which outlive restarts) are never
        // touched and prod stays up until the phase-4 restart.
        rsync_server_binaries(
            gcp,
            data_dir,
            &artifacts.server,
            &artifacts.relay,
            &artifacts.database,
            &artifacts.bot_manager,
        )?;
        if artifacts.built {
            write_infra_hash(paths)?;
        }
        println!("✅ Server binaries shipped ({unit})");
    } else {
        println!("==> Server binaries unchanged — skipping ship");
    }

    gcp.sync_file(&paths.version_file, &format!("{data_dir}/.version"))?;

    let deployed_cache = paths.deployed_version_cache(unit);
    let last_version = fs::read_to_string(&deployed_cache)
        .unwrap_or_default()
        .trim()
        .to_string();
    let version_changed = last_version != version;

    Ok(ServerShipResult {
        shipped_binaries: need_ship,
        version_changed,
    })
}

/// Phase 4: restart unit when binaries or version changed.
pub fn restart_server_if_needed(
    paths: &Paths,
    gcp: &GcpConfig,
    unit: &str,
    version: &str,
    ship: &ServerShipResult,
) -> Result<()> {
    if ship.shipped_binaries || ship.version_changed {
        println!("==> Restarting {unit}");
        gcp.run_remote(&format!("sudo systemctl restart {unit}"))?;
        let deployed_cache = paths.deployed_version_cache(unit);
        if let Some(parent) = deployed_cache.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&deployed_cache, format!("{version}\n"))?;
    } else {
        println!("==> Server version unchanged — skipping restart");
    }
    Ok(())
}
