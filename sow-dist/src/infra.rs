use crate::config::DeployConfig;
use crate::gcp::{self, GcpConfig};
use crate::paths::Paths;
use crate::process;
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const SERVER_CRATES: &[&str] = &["sow-server", "sow-relay", "sow-core", "sow-net"];

pub fn deploy_infra(
    paths: &Paths,
    cfg: &DeployConfig,
    confirm_destroy: bool,
    bootstrap_only: bool,
) -> Result<()> {
    let project = &cfg.gcp_project;
    gcp::enable_os_login(project)?;
    let gcp = cfg.gcp();
    if bootstrap_only {
        bootstrap_fedora(paths, cfg, &gcp)?;
    } else {
        if !confirm_destroy {
            bail!("Refusing to destroy/recreate VPS without --confirm-destroy (or use --bootstrap-only)");
        }
        gcp::delete_instance(project, &cfg.gcp_zone, &cfg.gcp_instance)?;
        if let (Some(name), Some(zone)) = (&cfg.test_instance, &cfg.test_zone) {
            gcp::delete_instance(project, zone, name)?;
        }
        if let (Some(name), Some(region)) = (&cfg.test_static_ip, &cfg.test_static_ip_region) {
            gcp::release_static_ip(project, region, name)?;
        }
        gcp::create_fedora_vm(
            project,
            &cfg.gcp_zone,
            &cfg.gcp_instance,
            &cfg.gcp_static_ip,
        )?;
        fs::remove_file(paths.remote_home_cache()).ok();
        wait_for_ssh(&gcp)?;
        bootstrap_fedora(paths, cfg, &gcp)?;
    }
    println!(
        "✅ Fedora VPS ready on {} ({})",
        cfg.gcp_instance, cfg.gcp_static_ip
    );
    Ok(())
}

fn wait_for_ssh(gcp: &GcpConfig) -> Result<()> {
    const MAX_SECS: u64 = 180;
    const INTERVAL_SECS: u64 = 5;
    println!("==> Waiting for VM SSH (up to {MAX_SECS}s)…");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(MAX_SECS);
    while std::time::Instant::now() < deadline {
        if gcp.ssh_ready() {
            println!("✅ SSH ready");
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(INTERVAL_SECS));
    }
    bail!("SSH not ready after {MAX_SECS}s")
}

/// Fedora 44 OS Login may not populate google-sudoers; grant sudo via one-shot startup script.
fn ensure_os_admin_sudo(gcp: &GcpConfig) -> Result<()> {
    if gcp
        .remote_output("sudo -n whoami")
        .map(|s| s.trim() == "root")
        .unwrap_or(false)
    {
        return Ok(());
    }
    let user = gcp.remote_output("whoami")?.trim().to_string();
    println!("==> Enabling passwordless sudo for {user} (startup script)");
    let script = format!(
        "#!/bin/bash\nset -euo pipefail\n\
         gpasswd --add {user} google-sudoers 2>/dev/null || true\n\
         echo '{user} ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/99-sow-deploy\n\
         chmod 0440 /etc/sudoers.d/99-sow-deploy\n"
    );
    let tmp = std::env::temp_dir().join("sow-sudo-bootstrap.sh");
    fs::write(&tmp, script)?;
    process::run(
        "gcloud",
        &[
            "compute",
            "instances",
            "add-metadata",
            &gcp.instance,
            &format!("--project={}", gcp.project),
            &format!("--zone={}", gcp.zone),
            &format!("--metadata-from-file=startup-script={}", tmp.display()),
        ],
        None,
    )?;
    process::run(
        "gcloud",
        &[
            "compute",
            "instances",
            "reset",
            &gcp.instance,
            &format!("--project={}", gcp.project),
            &format!("--zone={}", gcp.zone),
            "--quiet",
        ],
        None,
    )?;
    fs::remove_file(&tmp).ok();
    wait_for_ssh(gcp)?;
    if !gcp
        .remote_output("sudo -n whoami")
        .map(|s| s.trim() == "root")
        .unwrap_or(false)
    {
        bail!("sudo still unavailable after startup script — check OS Login / IAM");
    }
    Ok(())
}

fn bootstrap_fedora(paths: &Paths, cfg: &DeployConfig, gcp: &GcpConfig) -> Result<()> {
    ensure_os_admin_sudo(gcp)?;
    let login_home = gcp.remote_home(&paths.remote_home_cache())?;
    let user = gcp.remote_output("whoami")?.trim().to_string();
    let home_prod = format!("{login_home}/shadowsofwar");
    let home_ptr = format!("{login_home}/shadowsofwar-ptr");

    println!("==> Bootstrap Fedora (user={user}, prod={home_prod})");

    gcp.run_remote(
        "sudo dnf -y install nginx valkey certbot python3-certbot-nginx firewalld && \
         sudo systemctl enable --now firewalld && \
         sudo firewall-cmd --permanent --add-service=http --add-service=https && \
         sudo firewall-cmd --reload && \
         sudo setsebool -P httpd_can_network_connect 1",
    )?;

    let web_main = cfg.web_root_main();
    let web_play = cfg.web_root_play();
    let web_ptr = cfg.web_root_ptr();
    gcp.run_remote(&format!(
        "mkdir -p {home_prod}/assets/maps {home_ptr}/assets/maps && \
         sudo mkdir -p {web_main} {web_play} {web_ptr} && \
         sudo chown -R {user}:$(id -gn) /var/www/{main} /var/www/{play} /var/www/{ptr} && \
         sudo restorecon -R /var/www",
        main = cfg.site_domain(),
        play = cfg.play_domain(),
        ptr = cfg.ptr_domain(),
    ))?;

    install_systemd_unit(gcp, paths, "sow-server.service", &user, &home_prod, &home_ptr)?;
    install_systemd_unit(gcp, paths, "sow-server-ptr.service", &user, &home_prod, &home_ptr)?;
    install_systemd_unit(gcp, paths, "valkey.service", &user, &home_prod, &home_ptr)?;

    for (template, conf_name) in [
        ("main.conf", format!("{}.conf", cfg.site_domain())),
        ("play.conf", format!("{}.conf", cfg.play_domain())),
        ("ptr.conf", format!("{}.conf", cfg.ptr_domain())),
    ] {
        install_nginx_conf(gcp, paths, cfg, &template, &conf_name)?;
    }

    let certbot = format!(
        "sudo nginx -t && sudo systemctl enable --now nginx valkey && \
         sudo certbot --nginx --non-interactive --agree-tos --email {email} \
         -d {main} -d {www} -d {play} -d {ptr} --redirect || true",
        email = cfg.certbot_email,
        main = cfg.site_domain(),
        www = cfg.www_site_domain(),
        play = cfg.play_domain(),
        ptr = cfg.ptr_domain(),
    );
    gcp.run_remote(&certbot)?;

    gcp.run_remote("sudo systemctl daemon-reload && sudo systemctl enable --now sow-server sow-server-ptr")?;

    Ok(())
}

fn install_nginx_conf(
    gcp: &GcpConfig,
    paths: &Paths,
    cfg: &DeployConfig,
    template: &str,
    conf_name: &str,
) -> Result<()> {
    let mut content = fs::read_to_string(paths.deploy_nginx(template))?;
    content = content.replace("__SOW_DOMAIN_MAIN__", &cfg.site_domain());
    content = content.replace("__SOW_WWW_MAIN__", &cfg.www_site_domain());
    content = content.replace("__SOW_DOMAIN_PLAY__", &cfg.play_domain());
    content = content.replace("__SOW_DOMAIN_PTR__", &cfg.ptr_domain());
    let tmp = paths.dist_root().join(format!("bootstrap-nginx-{conf_name}"));
    fs::write(&tmp, &content)?;
    let remote = format!("/tmp/{conf_name}");
    gcp::scp_to_instance(gcp, &tmp, &remote)?;
    gcp.run_remote(&format!(
        "sudo mv {remote} /etc/nginx/conf.d/{conf_name} && \
         sudo chown root:root /etc/nginx/conf.d/{conf_name} && sudo chmod 644 /etc/nginx/conf.d/{conf_name} && \
         sudo restorecon /etc/nginx/conf.d/{conf_name}"
    ))?;
    fs::remove_file(&tmp).ok();
    Ok(())
}

fn install_systemd_unit(
    gcp: &GcpConfig,
    paths: &Paths,
    name: &str,
    user: &str,
    home_prod: &str,
    home_ptr: &str,
) -> Result<()> {
    let mut content = fs::read_to_string(paths.deploy_systemd(name))?;
    content = content.replace("__DEPLOY_USER__", user);
    content = content.replace("__HOME_PROD__", home_prod);
    content = content.replace("__HOME_PTR__", home_ptr);
    let tmp = paths.dist_root().join(format!("bootstrap-{name}"));
    fs::write(&tmp, &content)?;
    let remote = format!("/tmp/{name}");
    gcp::scp_to_instance(gcp, &tmp, &remote)?;
    gcp.run_remote(&format!(
        "sudo mv {remote} /etc/systemd/system/{name} && \
         sudo chown root:root /etc/systemd/system/{name} && sudo chmod 644 /etc/systemd/system/{name}"
    ))?;
    fs::remove_file(&tmp).ok();
    Ok(())
}

#[derive(Clone, Debug)]
pub struct ServerArtifacts {
    pub server: PathBuf,
    pub relay: PathBuf,
    pub built: bool,
}

#[derive(Clone, Debug)]
pub struct ServerShipResult {
    pub shipped_binaries: bool,
    pub version_changed: bool,
}

pub fn local_server_binaries(paths: &Paths) -> (PathBuf, PathBuf) {
    const GNU: &str = "x86_64-unknown-linux-gnu";
    let dir = paths.cargo_target.join(format!("{GNU}/release"));
    (dir.join("sow-server"), dir.join("sow-relay"))
}

pub fn needs_local_server_build(paths: &Paths) -> Result<bool> {
    let current_hash = hash_server_inputs(paths)?;
    let hash_changed = read_cached_hash(paths) != current_hash;
    let (server, relay) = local_server_binaries(paths);
    Ok(hash_changed || !server.is_file() || !relay.is_file())
}

/// Phase 1: compile server binaries locally when crate inputs changed.
pub fn build_server_if_needed(paths: &Paths) -> Result<ServerArtifacts> {
    let (server, relay) = local_server_binaries(paths);
    if needs_local_server_build(paths)? {
        let (server, relay) = build_server_binaries(paths)?;
        Ok(ServerArtifacts {
            server,
            relay,
            built: true,
        })
    } else {
        println!("==> Server crates unchanged — skipping server build");
        Ok(ServerArtifacts {
            server,
            relay,
            built: false,
        })
    }
}

pub fn remote_binaries_missing(gcp: &GcpConfig, data_dir: &str) -> bool {
    gcp.remote_output(&format!(
        "test -x {data_dir}/sow-server && test -x {data_dir}/sow-relay && echo ok"
    ))
    .map(|s| s.trim() != "ok")
    .unwrap_or(true)
}

/// Phase 3: rsync server binaries and `.version` (no restart).
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
        rsync_server_binaries(
            gcp,
            &paths.dist_root(),
            data_dir,
            &artifacts.server,
            &artifacts.relay,
        )?;
        if artifacts.built {
            write_infra_hash(paths)?;
        }
        println!("✅ Server binaries shipped ({unit})");
    } else {
        println!("==> Server binaries unchanged — skipping ship");
    }

    let version_local = paths.version_file.to_string_lossy();
    gcp.rsync(
        &paths.dist_root(),
        version_local.as_ref(),
        &format!("{data_dir}/.version"),
    )?;

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

fn read_cached_hash(paths: &Paths) -> String {
    fs::read_to_string(paths.infra_hash_cache())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn build_server_binaries(paths: &Paths) -> Result<(PathBuf, PathBuf)> {
    const GNU: &str = "x86_64-unknown-linux-gnu";
    println!("==> cargo build --release -p sow-server -p sow-relay ({GNU})");
    process::run(
        "cargo",
        &[
            "build",
            "--release",
            "-p",
            "sow-server",
            "-p",
            "sow-relay",
            "--target",
            GNU,
        ],
        Some(&paths.root),
    )?;
    let dir = paths.cargo_target.join(format!("{GNU}/release"));
    Ok((dir.join("sow-server"), dir.join("sow-relay")))
}

fn rsync_server_binaries(
    gcp: &GcpConfig,
    cache_dir: &Path,
    data_dir: &str,
    server: &Path,
    relay: &Path,
) -> Result<()> {
    gcp.rsync(
        cache_dir,
        &server.to_string_lossy(),
        &format!("{data_dir}/sow-server"),
    )?;
    gcp.rsync(
        cache_dir,
        &relay.to_string_lossy(),
        &format!("{data_dir}/sow-relay"),
    )?;
    gcp.run_remote(&format!(
        "chmod +x {data_dir}/sow-server {data_dir}/sow-relay && \
         sudo chcon -t bin_t {data_dir}/sow-server {data_dir}/sow-relay"
    ))?;
    Ok(())
}

pub fn verify_server_health(
    gcp: &GcpConfig,
    maps_url: &str,
    ws_url: &str,
    unit: &str,
) -> Result<()> {
    println!("==> Verifying server ({unit}) on VPS");
    let active = gcp.remote_output(&format!("systemctl is-active {unit}"))?;
    if active.trim() != "active" {
        let logs = gcp
            .remote_output(&format!("journalctl -u {unit} -n 30 --no-pager"))
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
    if cors != "*" {
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
