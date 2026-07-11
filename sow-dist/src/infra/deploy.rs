use crate::config::DeployConfig;
use crate::gcp::{self, GcpConfig};
use crate::paths::Paths;
use anyhow::{bail, Result};
use std::fs;

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
        bootstrap_debian(paths, cfg, &gcp)?;
    } else {
        if !confirm_destroy {
            bail!("Refusing to destroy/recreate VPS without --confirm-destroy (or use --bootstrap-only)");
        }
        // TODO: don't destroy — keep old VMs for forensics/analytics/backup
        gcp::delete_instance(project, &cfg.gcp_zone, &cfg.gcp_instance)?;
        if let (Some(name), Some(zone)) = (&cfg.test_instance, &cfg.test_zone) {
            gcp::delete_instance(project, zone, name)?;
        }
        if let (Some(name), Some(region)) = (&cfg.test_static_ip, &cfg.test_static_ip_region) {
            gcp::release_static_ip(project, region, name)?;
        }
        gcp::create_debian_vm(
            project,
            &cfg.gcp_zone,
            &cfg.gcp_instance,
            &cfg.gcp_static_ip,
        )?;
        fs::remove_file(paths.remote_home_cache()).ok();
        wait_for_ssh(&gcp)?;
        bootstrap_debian(paths, cfg, &gcp)?;
    }
    println!(
        "✅ Debian VPS ready on {} ({})",
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

/// Debian 13 on GCP uses OS Login; sudo is granted via roles/compute.osAdminLogin.
/// No SELinux, no firewalld — ufw handles the host firewall.
fn bootstrap_debian(paths: &Paths, cfg: &DeployConfig, gcp: &GcpConfig) -> Result<()> {
    let login_home = gcp.remote_home(&paths.remote_home_cache())?;
    let user = gcp.remote_output("whoami")?.trim().to_string();
    let home_prod = format!("{login_home}/shadowsofwar");
    let home_ptr = format!("{login_home}/shadowsofwar-ptr");

    println!("==> Bootstrap Debian 13 (user={user}, prod={home_prod})");

    // Install packages — valkey is in Debian 13 (trixie) main
    gcp.run_remote(
        "sudo apt-get update -qq && \
         sudo apt-get install -y nginx valkey certbot python3-certbot-nginx ufw",
    )?;

    // Host firewall: allow SSH from GCP IAP only, and web traffic publicly.
    // GCP firewall is the outer layer, but UFW adds defense-in-depth on the host.
    gcp.run_remote(
        "sudo ufw allow proto tcp from 35.235.240.0/20 to any port 22 && \
         sudo ufw allow http && \
         sudo ufw allow https && \
         echo 'y' | sudo ufw enable",
    )?;

    let web_main = cfg.web_root_main();
    let web_play = cfg.web_root_play();
    let web_ptr = cfg.web_root_ptr();
    gcp.run_remote(&format!(
        "mkdir -p {home_prod}/assets/maps {home_ptr}/assets/maps && \
         sudo mkdir -p {web_main} {web_play} {web_ptr} && \
         sudo chown -R {user}:$(id -gn) /var/www/{main} /var/www/{play} /var/www/{ptr}",
        main = cfg.site_domain(),
        play = cfg.play_domain(),
        ptr = cfg.ptr_domain(),
    ))?;

    // Ensure default environment files with placeholders exist but do not overwrite them
    gcp.run_remote(
        "sudo mkdir -p /etc/default && \
         sudo touch /etc/default/shadowsofwar /etc/default/shadowsofwar-ptr && \
         (test -s /etc/default/shadowsofwar || echo -e 'SOW_DB_SECRET=REPLACE_WITH_SOW_DB_SECRET\\nCRAZYGAMES_API_KEY=REPLACE_WITH_CRAZYGAMES_API_KEY' | sudo tee /etc/default/shadowsofwar >/dev/null) && \
         (test -s /etc/default/shadowsofwar-ptr || echo -e 'SOW_DB_SECRET=REPLACE_WITH_SOW_DB_SECRET\\nCRAZYGAMES_API_KEY=REPLACE_WITH_CRAZYGAMES_API_KEY' | sudo tee /etc/default/shadowsofwar-ptr >/dev/null) && \
         sudo chmod 600 /etc/default/shadowsofwar /etc/default/shadowsofwar-ptr"
    )?;

    install_systemd_unit(
        gcp,
        paths,
        cfg,
        "sow-server.service",
        &user,
        &home_prod,
        &home_ptr,
    )?;
    install_systemd_unit(
        gcp,
        paths,
        cfg,
        "sow-server-ptr.service",
        &user,
        &home_prod,
        &home_ptr,
    )?;
    install_systemd_unit(
        gcp,
        paths,
        cfg,
        "sow-database.service",
        &user,
        &home_prod,
        &home_ptr,
    )?;
    install_systemd_unit(
        gcp,
        paths,
        cfg,
        "sow-database-ptr.service",
        &user,
        &home_prod,
        &home_ptr,
    )?;
    install_systemd_unit(
        gcp,
        paths,
        cfg,
        "valkey.service",
        &user,
        &home_prod,
        &home_ptr,
    )?;


    for (template, conf_name) in [
        ("main.conf", format!("{}.conf", cfg.site_domain())),
        ("play.conf", format!("{}.conf", cfg.play_domain())),
        ("ptr.conf", format!("{}.conf", cfg.ptr_domain())),
    ] {
        install_nginx_conf(gcp, paths, cfg, template, &conf_name)?;
    }

    let certbot = format!(
        "sudo nginx -t && sudo systemctl enable --now nginx valkey && \
         sudo certbot --nginx --non-interactive --agree-tos --email {email} \
         -d {main} -d {www} -d {play} -d {ptr} --redirect",
        email = cfg.certbot_email,
        main = cfg.site_domain(),
        www = cfg.www_site_domain(),
        play = cfg.play_domain(),
        ptr = cfg.ptr_domain(),
    );
    gcp.run_remote(&certbot)?;

    gcp.run_remote(
        "sudo systemctl daemon-reload && sudo systemctl enable --now sow-server sow-server-ptr sow-database sow-database-ptr",
    )?;

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
    content = content.replace("__SOW_PROD_WS_PORT__", &cfg.prod_ws_port());
    content = content.replace("__SOW_PROD_MAPS_PORT__", &cfg.prod_maps_port());
    content = content.replace("__SOW_PROD_DB_PORT__", &cfg.prod_db_port());
    content = content.replace("__SOW_PTR_WS_PORT__", &cfg.ptr_ws_port());
    content = content.replace("__SOW_PTR_MAPS_PORT__", &cfg.ptr_maps_port());
    content = content.replace("__SOW_PTR_DB_PORT__", &cfg.ptr_db_port());
    let tmp = paths
        .dist_root()
        .join(format!("bootstrap-nginx-{conf_name}"));
    fs::write(&tmp, &content)?;
    let remote = format!("/tmp/{conf_name}");
    gcp::scp_to_instance(gcp, &tmp, &remote)?;
    gcp.run_remote(&format!(
        "sudo mv {remote} /etc/nginx/conf.d/{conf_name} && \
         sudo chown root:root /etc/nginx/conf.d/{conf_name} && sudo chmod 644 /etc/nginx/conf.d/{conf_name}"
    ))?;
    fs::remove_file(&tmp).ok();
    Ok(())
}

fn install_systemd_unit(
    gcp: &GcpConfig,
    paths: &Paths,
    cfg: &DeployConfig,
    name: &str,
    user: &str,
    home_prod: &str,
    home_ptr: &str,
) -> Result<()> {
    let mut content = fs::read_to_string(paths.deploy_systemd(name))?;
    content = content.replace("__DEPLOY_USER__", user);
    content = content.replace("__HOME_PROD__", home_prod);
    content = content.replace("__HOME_PTR__", home_ptr);
    content = content.replace("__SOW_PROD_WS_PORT__", &cfg.prod_ws_port());
    content = content.replace("__SOW_PROD_MAPS_PORT__", &cfg.prod_maps_port());
    content = content.replace("__SOW_PROD_DB_PORT__", &cfg.prod_db_port());
    content = content.replace("__SOW_PTR_WS_PORT__", &cfg.ptr_ws_port());
    content = content.replace("__SOW_PTR_MAPS_PORT__", &cfg.ptr_maps_port());
    content = content.replace("__SOW_PTR_DB_PORT__", &cfg.ptr_db_port());
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

pub fn deploy_configs_if_needed(paths: &Paths, cfg: &DeployConfig) -> Result<()> {
    use super::hash::hash_deploy_templates;

    let current_hash = hash_deploy_templates(paths)?;
    let cache_file = paths.state_dir().join("deploy-config-hash");
    let last_hash = fs::read_to_string(&cache_file)
        .unwrap_or_default()
        .trim()
        .to_string();

    if current_hash == last_hash {
        println!("==> VPS configurations (Nginx/systemd) unchanged — skipping config deploy");
        return Ok(());
    }

    if cfg.certbot_email.trim().is_empty() {
        eprintln!(
            "==> VPS deploy templates changed but SOW_CERTBOT_EMAIL is unset — skipping nginx/systemd push"
        );
        eprintln!(
            "    (avoids stripping TLS; set SOW_CERTBOT_EMAIL in sow-dist/.env and re-run ./sow p)"
        );
        return Ok(());
    }

    println!("==> VPS configuration changes detected! Deploying Nginx configurations and systemd units...");

    let gcp = cfg.gcp();
    let login_home = gcp.remote_home(&paths.remote_home_cache())?;
    let user = gcp.remote_output("whoami")?.trim().to_string();
    let home_prod = format!("{login_home}/shadowsofwar");
    let home_ptr = format!("{login_home}/shadowsofwar-ptr");

    for unit_name in &[
        "sow-server.service",
        "sow-server-ptr.service",
        "sow-database.service",
        "sow-database-ptr.service",
        "valkey.service",
    ] {
        install_systemd_unit(&gcp, paths, cfg, unit_name, &user, &home_prod, &home_ptr)?;
    }

    for (template, conf_name) in [
        ("main.conf", format!("{}.conf", cfg.site_domain())),
        ("play.conf", format!("{}.conf", cfg.play_domain())),
        ("ptr.conf", format!("{}.conf", cfg.ptr_domain())),
    ] {
        install_nginx_conf(&gcp, paths, cfg, template, &conf_name)?;
    }

    println!("==> Reloading systemd daemon on VPS...");
    gcp.run_remote("sudo systemctl daemon-reload")?;

    println!("==> Re-running Certbot to ensure SSL on new config files...");
    let certbot = format!(
        "sudo certbot --nginx --non-interactive --agree-tos --email {email} \
         -d {main} -d {www} -d {play} -d {ptr} --redirect",
        email = cfg.certbot_email,
        main = cfg.site_domain(),
        www = cfg.www_site_domain(),
        play = cfg.play_domain(),
        ptr = cfg.ptr_domain(),
    );
    gcp.run_remote(&certbot)?;

    println!("==> Testing and reloading Nginx on VPS...");
    gcp.run_remote("sudo nginx -t && sudo systemctl reload nginx")?;

    if let Some(parent) = cache_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&cache_file, format!("{current_hash}\n"))?;
    println!("✅ VPS configurations deployed successfully.");

    Ok(())
}
