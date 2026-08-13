use super::*;

const RELAY_HOST: &str = "relay";
const RELAY_ROOT: &str = "/home/azureuser/shadows-of-war";
const RELAY_USER: &str = "sowrelay";
const RELAY_GROUP: &str = "sowrelay";
const RELAY_EXEC: &str = "/usr/local/libexec/sow-relay/sow-relay";
const RELAY_CONFIG: &str = "/usr/local/etc/sow/echo-vf.ini";
const RELAY_STATE: &str = "/var/lib/sow-relay";
const RELAY_REPLAYS: &str = "/var/lib/sow-relay/replays";

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

pub(super) fn execute(paths: &Paths) -> Result<()> {
    let host = env_or("SOW_RELAY_DEPLOY_HOST", RELAY_HOST);
    let root = env_or("SOW_RELAY_ROOT", RELAY_ROOT);
    let db_secret = env::var("SOW_DB_SECRET")
        .context("SOW_DB_SECRET must be provided via ignored sow-dist/.env")?;
    if db_secret.trim().is_empty() {
        bail!("SOW_DB_SECRET must not be empty");
    }
    let control_secret = env::var("SOW_RELAY_CONTROL_SECRET")
        .context("SOW_RELAY_CONTROL_SECRET must be provided via ignored sow-dist/.env")?;
    if control_secret.trim().is_empty() {
        bail!("SOW_RELAY_CONTROL_SECRET must not be empty");
    }
    let worker_count = env_or("SOW_RELAY_WORKER_COUNT", "4")
        .parse::<usize>()
        .context("SOW_RELAY_WORKER_COUNT must be an integer")?;
    if !(1..=64).contains(&worker_count) {
        bail!("SOW_RELAY_WORKER_COUNT must be between 1 and 64");
    }
    let tickets_required = env_or("SOW_RELAY_TICKETS_REQUIRED", "1");
    if tickets_required != "0" && tickets_required != "1" {
        bail!("SOW_RELAY_TICKETS_REQUIRED must be 0 or 1");
    }
    let max_connections = env_or("SOW_RELAY_MAX_CONNECTIONS", "32768")
        .parse::<usize>()
        .context("SOW_RELAY_MAX_CONNECTIONS must be an integer")?;
    let max_connections_per_ip = env_or("SOW_RELAY_MAX_CONNECTIONS_PER_IP", "4096")
        .parse::<usize>()
        .context("SOW_RELAY_MAX_CONNECTIONS_PER_IP must be an integer")?;
    let handshakes_per_ip = env_or("SOW_RELAY_HANDSHAKES_PER_IP", "512")
        .parse::<u32>()
        .context("SOW_RELAY_HANDSHAKES_PER_IP must be an integer")?;
    if max_connections == 0 || max_connections_per_ip == 0 || handshakes_per_ip == 0 {
        bail!("relay admission limits must be positive");
    }
    if max_connections_per_ip > max_connections {
        bail!("SOW_RELAY_MAX_CONNECTIONS_PER_IP must not exceed SOW_RELAY_MAX_CONNECTIONS");
    }
    let db_url = env_or("SOW_DB_URL", "https://shadowsofwar.io");
    let parsed_db_url = url::Url::parse(&db_url)
        .with_context(|| format!("invalid SOW_DB_URL={db_url}"))?;
    if parsed_db_url.scheme() != "https" {
        bail!("SOW_DB_URL must use https for relay production deploys");
    }
    let db_resolve_ip = env_or("SOW_DB_RESOLVE_IP", "74.208.246.177");
    db_resolve_ip
        .parse::<std::net::IpAddr>()
        .with_context(|| format!("invalid SOW_DB_RESOLVE_IP={db_resolve_ip}"))?;
    let secondary_ids = if worker_count > 1 {
        format!("{}", (1..worker_count).map(|id| id.to_string()).collect::<Vec<_>>().join("|"))
    } else {
        String::new()
    };
    let unit_wants = (0..worker_count)
        .map(|id| format!("sow-relay@{id}.service"))
        .collect::<Vec<_>>()
        .join(" ");
    let unit_stops = (0..worker_count)
        .rev()
        .map(|id| format!("sow-relay@{id}.service"))
        .collect::<Vec<_>>()
        .join(" ");
    let mgmt_ports = (0..worker_count)
        .map(|id| (8080 + id).to_string())
        .collect::<Vec<_>>();

    let build_check = format!(
        "export PATH=$HOME/.cargo/bin:$PATH; test -d {} && test -f {}/Cargo.toml && command -v cargo && test -f {}/fstack-bridge/build.rs",
        shell_quote(&root),
        shell_quote(&root),
        shell_quote(&root),
    );
    run("ssh", &[&host, &build_check], None).context("relay VM not ready")?;

    let source = format!("{}/", paths.root.display());
    let destination = format!("{host}:{root}/");
    run(
        "rsync",
        &[
            "-azc",
            "--delete",
            "--exclude=.git",
            "--exclude=dist",
            "--exclude=target",
            "--exclude=sow-dist/.env",
            "--exclude=replays",
            &source,
            &destination,
        ],
        Some(&paths.root),
    )?;

    let fstack_lib_dir = env_or("SOW_FSTACK_LIB_DIR", "/usr/local/lib");
    let build = format!(
        "set -eu; export PATH=$HOME/.cargo/bin:$PATH; export FSTACK_LIB_DIR={}; cd {}; cargo build --release -p sow-relay",
        shell_quote(&fstack_lib_dir),
        shell_quote(&root)
    );
    run("ssh", &[&host, &build], None)?;

    let root_q = shell_quote(&root);
    let runtime_setup = format!(
        "set -eu; \
         sudo systemctl stop sow-relay.service {unit_stops} 2>/dev/null || true; \
         if ! getent group {group} >/dev/null; then sudo groupadd --system {group}; fi; \
         if ! id -u {user} >/dev/null 2>&1; then sudo useradd --system --gid {group} --home-dir /nonexistent --shell /usr/sbin/nologin {user}; fi; \
         sudo install -d -o root -g {group} -m 0750 /usr/local/libexec/sow-relay /usr/local/etc/sow; \
         sudo install -d -o {user} -g {group} -m 0750 {state} {replays}; \
         sudo install -d -o {user} -g {group} -m 0700 /var/run/dpdk; \
         sudo chown -R {user}:{group} /var/run/dpdk /dev/hugepages; \
         sudo install -o root -g {group} -m 0750 {root_q}/target/release/sow-relay {exec}; \
         sudo install -o root -g {group} -m 0640 {root_q}/fstack-bridge/echo-vf.ini {config}; \
         sudo tee /etc/tmpfiles.d/sow-relay.conf >/dev/null <<'EOF'\n\
d /var/run/dpdk 0700 {user} {group} -\n\
Z /dev/hugepages 0700 {user} {group} -\n\
EOF\n\
         sudo install -d -m 0755 /etc/systemd/journald.conf.d; \
         sudo tee /etc/systemd/journald.conf.d/30-sow-relay.conf >/dev/null <<'EOF'\n\
[Journal]\n\
SystemMaxUse=256M\n\
RuntimeMaxUse=128M\n\
MaxRetentionSec=7day\n\
RateLimitIntervalSec=30s\n\
RateLimitBurst=10000\n\
EOF\n\
         sudo systemctl restart systemd-journald; \
         sudo journalctl --rotate; \
         sudo journalctl --vacuum-size=256M",
        unit_stops = unit_stops,
        user = RELAY_USER,
        group = RELAY_GROUP,
        state = RELAY_STATE,
        replays = RELAY_REPLAYS,
        root_q = root_q,
        exec = RELAY_EXEC,
        config = RELAY_CONFIG,
    );
    run("ssh", &[&host, &runtime_setup], None)?;

    let worker_script = format!(
        "sudo install -d -m 0755 /usr/local/sbin; \
         sudo tee /usr/local/sbin/sow-relay-worker >/dev/null <<'EOF'\n\
#!/bin/sh\n\
set -eu\n\
id=\"${{1:?worker id required}}\"\n\
case \"$id\" in\n\
  0) proc_type=primary ;;\n\
  {secondary_ids}) proc_type=secondary ;;\n\
  *) echo \"invalid worker id: $id\" >&2; exit 64 ;;\n\
esac\n\
exec {exec} --conf {config} --proc-type=\"$proc_type\" --proc-id=\"$id\"\n\
EOF\n\
         sudo chmod 0755 /usr/local/sbin/sow-relay-worker",
        exec = RELAY_EXEC,
        config = RELAY_CONFIG,
    );
    run("ssh", &[&host, &worker_script], None)?;

    let remote_secret = format!("/tmp/sow-db-secret-{}", std::process::id());
    let remote_control_secret = format!("/tmp/sow-relay-control-secret-{}", std::process::id());
    stage_secret(&host, &db_secret, &remote_secret)?;
    stage_secret(&host, &control_secret, &remote_control_secret)?;

    let drop_in = format!(
        "set -eu; secret=$(cat {}); control_secret=$(cat {}); rm -f {} {}; \
         sudo rm -f /etc/systemd/system/sow-relay.service.d/override.conf; \
         sudo rmdir /etc/systemd/system/sow-relay.service.d 2>/dev/null || true; \
         sudo mkdir -p /etc/systemd/system/sow-relay@.service.d && \
         sudo tee /etc/systemd/system/sow-relay@.service.d/override.conf >/dev/null <<EOF\n\
         [Service]\n\
         LimitNOFILE=1000000\n\
         Environment=RUST_LOG=info\n\
         Environment=SOW_MGMT_LISTEN=0.0.0.0\n\
         Environment=SOW_MGMT_TLS_REQUIRED=1\n\
         Environment=SOW_RELAY_WORKER_COUNT={}\n\
         Environment=SOW_RELAY_TICKETS_REQUIRED={}\n\
         Environment=SOW_RELAY_MAX_CONNECTIONS={}\n\
         Environment=SOW_RELAY_MAX_CONNECTIONS_PER_IP={}\n\
         Environment=SOW_RELAY_HANDSHAKES_PER_IP={}\n\
         Environment=SOW_DB_URL={}\n\
         Environment=SOW_DB_RESOLVE_IP={}\n\
         Environment=SOW_REPLAY_SPOOL_DIR={}\n\
         Environment=SOW_DB_SECRET=$secret\n\
         Environment=SOW_RELAY_CONTROL_SECRET=$control_secret\n\
         Environment=SOW_RELAY_TLS_CERT=/usr/local/etc/sow/relay.crt\n\
         Environment=SOW_RELAY_TLS_KEY=/usr/local/etc/sow/relay.key\n\
         EOF\n\
         sudo chmod 0600 /etc/systemd/system/sow-relay@.service.d/override.conf",
        shell_quote(&remote_secret),
        shell_quote(&remote_control_secret),
        shell_quote(&remote_secret),
        shell_quote(&remote_control_secret),
        worker_count,
        tickets_required,
        max_connections,
        max_connections_per_ip,
        handshakes_per_ip,
        shell_quote(&db_url),
        shell_quote(&db_resolve_ip),
        shell_quote(RELAY_REPLAYS),
    );
    run("ssh", &[&host, &drop_in], None)?;

    // Deploy TLS cert + key to the relay VM if they exist locally.
    let cert_local = env_or("SOW_RELAY_TLS_CERT_LOCAL", "/etc/letsencrypt/live/relay.shadowsofwar.io/fullchain.pem");
    let key_local = env_or("SOW_RELAY_TLS_KEY_LOCAL", "/etc/letsencrypt/live/relay.shadowsofwar.io/privkey.pem");
    if Path::new(&cert_local).exists() && Path::new(&key_local).exists() {
        run("scp", &["-q", &cert_local, &format!("{host}:/tmp/relay-fullchain.pem")], None)?;
        run("scp", &["-q", &key_local, &format!("{host}:/tmp/relay-privkey.pem")], None)?;
        let install = format!(
            "sudo install -d -o root -g sowrelay -m 0750 /usr/local/etc/sow && \
             sudo install -o root -g sowrelay -m 0640 /tmp/relay-fullchain.pem /usr/local/etc/sow/relay.crt && \
             sudo install -o root -g sowrelay -m 0640 /tmp/relay-privkey.pem /usr/local/etc/sow/relay.key"
        );
        run("ssh", &[&host, &install], None)?;
        println!("✅ TLS cert deployed to {host} (/usr/local/etc/sow/)");
    } else {
        let remote_tls = output(
            "ssh",
            &[
                &host,
                "sudo test -s /usr/local/etc/sow/relay.crt && sudo test -s /usr/local/etc/sow/relay.key",
            ],
        )
        .is_ok();
        if remote_tls {
            let fix_tls_perms = "sudo chown root:sowrelay /usr/local/etc/sow/relay.crt /usr/local/etc/sow/relay.key && sudo chmod 0640 /usr/local/etc/sow/relay.crt /usr/local/etc/sow/relay.key";
            run("ssh", &[&host, fix_tls_perms], None)?;
            println!("✅ Existing remote TLS certificate retained on {host}");
        } else {
            println!("⚠️  TLS cert not found locally or remotely — relay will run plain ws://");
        }
    }

    let worker_unit = format!(
        "sudo tee /etc/systemd/system/sow-relay@.service >/dev/null <<'EOF'\n\
         [Unit]\n\
         Description=SOW F-Stack relay worker %i\n\
         After=network-online.target systemd-tmpfiles-setup.service\n\
         Wants=network-online.target\n\
         PartOf=sow-relay.service\n\
         [Service]\n\
         Type=simple\n\
         User={user}\n\
         Group={group}\n\
         UMask=0077\n\
         LimitNOFILE=1000000\n\
         LimitMEMLOCK=infinity\n\
         CapabilityBoundingSet=CAP_IPC_LOCK CAP_NET_ADMIN CAP_NET_RAW CAP_SYS_NICE\n\
         AmbientCapabilities=CAP_IPC_LOCK CAP_NET_ADMIN CAP_NET_RAW CAP_SYS_NICE\n\
         NoNewPrivileges=yes\n\
         PrivateTmp=yes\n\
         ProtectHome=yes\n\
         ProtectSystem=full\n\
         RuntimeDirectory=sow-relay\n\
         RuntimeDirectoryMode=0700\n\
         Environment=RUNTIME_DIRECTORY=/run/sow-relay\n\
         WorkingDirectory={state}\n\
         ExecStart=/usr/local/sbin/sow-relay-worker %i\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         ExecStartPre=/bin/sh -c 'test \"%i\" = 0 || until curl -kfsS --max-time 1 https://127.0.0.1:8080/healthz >/dev/null; do sleep 1; done'\n\
         [Install]\n\
         WantedBy=sow-relay.service\n\
         EOF",
        user = RELAY_USER,
        group = RELAY_GROUP,
        state = RELAY_STATE,
    );
    run("ssh", &[&host, &worker_unit], None)?;

    let aggregate_unit = format!("sudo tee /etc/systemd/system/sow-relay.service >/dev/null <<'EOF'\n[Unit]\nDescription=SOW F-Stack relay worker group\nWants={unit_wants}\nAfter=network-online.target\n[Service]\nType=oneshot\nRemainAfterExit=yes\nExecStart=/bin/true\nExecStop=/usr/bin/systemctl stop {unit_stops}\n[Install]\nWantedBy=multi-user.target\nEOF");
    run("ssh", &[&host, &aggregate_unit], None)?;

    let restart = format!("sudo systemctl disable --now sow-relay.service 2>/dev/null || true; sudo systemctl stop {unit_stops} 2>/dev/null || true; sudo systemctl daemon-reload; sudo systemctl enable sow-relay.service; sudo systemctl start sow-relay.service; sleep 3; sudo systemctl is-active sow-relay.service {unit_wants}");
    run("ssh", &[&host, &restart], None)?;

    let verify = format!(
        "set -eu; \
         sudo test -s /usr/local/etc/sow/relay.crt; \
         sudo test -s /usr/local/etc/sow/relay.key; \
         test \"$(sudo stat -c '%a %U:%G' /usr/local/libexec/sow-relay/sow-relay)\" = '750 root:sowrelay'; \
         test \"$(sudo stat -c '%a %U:%G' /usr/local/etc/sow/echo-vf.ini)\" = '640 root:sowrelay'; \
         test \"$(sudo stat -c '%a %U:%G' /usr/local/etc/sow/relay.key)\" = '640 root:sowrelay'; \
         for u in {}; do \
             test \"$(systemctl show $u -p User --value)\" = sowrelay; \
             test \"$(systemctl show $u -p NoNewPrivileges --value)\" = yes; \
         done; \
         sudo -u sowrelay test -r /usr/local/etc/sow/relay.key; \
         sudo -u sowrelay test -w /var/lib/sow-relay/replays; \
         sudo -u sowrelay test -w /dev/hugepages; \
         test ! -e /etc/systemd/system/sow-relay.service.d/override.conf; \
         test \"$(stat -c '%a' /etc/systemd/system/sow-relay@.service.d/override.conf)\" = 600; \
         if sudo grep -R -l -E 'sow_db_dev_secret_123_change_me_in_prod|SOW_DB_URL=http://' /etc/systemd/system /etc/sow 2>/dev/null | grep -q .; then \
             echo 'legacy relay secret or HTTP DB URL remains on relay' >&2; exit 78; \
         fi; \
         for p in {}; do curl -kfsS --max-time 5 https://127.0.0.1:$p/healthz >/dev/null; echo mgmt-$p-tls-ok; done; \
         pgrep -af sow-relay; \
         for u in {}; do \
             test \"$(systemctl show $u -p ActiveState --value)\" = active; \
             test \"$(systemctl show $u -p Result --value)\" = success; \
             test \"$(systemctl show $u -p ExecMainStatus --value)\" = 0; \
         done",
        unit_wants,
        mgmt_ports.join(" "),
        unit_wants
    );
    run("ssh", &[&host, &verify], None)?;

    println!("✅ relay deployed to {host}");
    Ok(())
}
