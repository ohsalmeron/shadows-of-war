use super::*;
use hmac::{Hmac, Mac};
use serde_json::json;
use std::collections::BTreeMap;

const BUILD_HOST: &str = "freebsd";
const BUILD_ROOT: &str = "/home/bizkit/shadows-of-war";
const CONTROL_HOST: &str = "ionos";
const RELAY_HOST: &str = "relay";
const RELAY_ROOT: &str = "/home/azureuser/shadows-of-war";
const RELAY_FSTACK_ROOT: &str = "/home/azureuser/f-stack-src";
const RELAY_USER: &str = "sowrelay";
const RELAY_GROUP: &str = "sowrelay";
const RELAY_EXEC: &str = "/usr/local/libexec/sow-relay/sow-relay";
const RELAY_CONFIG: &str = "/usr/local/etc/sow/echo-vf.ini";
const RELAY_STATE: &str = "/var/lib/sow-relay";
const RELAY_REPLAYS: &str = "/var/lib/sow-relay/replays";
const RELAY_MANIFEST: &str = "/var/lib/sow-relay/manifest.json";
const RELAY_STAGE: &str = "/home/azureuser/.sow-deploy/relay";
const FSTACK_LOCAL_REPO: &str = "/home/bizkit/Github/infra/f-stack";
const REMOTE_STAGE: &str = "/home/bizkit/.sow-deploy";
const REMOTE_RELEASES: &str = "/srv/sow/releases";
const PUBLIC_ORIGIN: &str = "https://shadowsofwar.io";
const SERVER_JAIL_IP: &str = "127.0.0.1";
const DATABASE_JAIL_IP: &str = "127.0.0.1";

struct Config {
    build_host: String,
    build_root: String,
    control_host: String,
    relay_host: String,
    relay_root: String,
    fstack_repo: String,
    remote_stage: String,
    public_origin: String,
    require_public: bool,
}

impl Config {
    fn load() -> Self {
        Self {
            build_host: env_or_alias("SOW_FREEBSD_BUILDER_HOST", "SOW_BUILD_HOST", BUILD_HOST),
            build_root: env_or("SOW_FREEBSD_BUILDER_ROOT", BUILD_ROOT),
            control_host: env_or_alias("SOW_CONTROL_HOST", "SOW_PROD_HOST", CONTROL_HOST),
            relay_host: env_or("SOW_RELAY_DEPLOY_HOST", RELAY_HOST),
            relay_root: env_or("SOW_RELAY_ROOT", RELAY_ROOT),
            fstack_repo: env_or("SOW_FSTACK_REPO", FSTACK_LOCAL_REPO),
            remote_stage: env_or("SOW_REMOTE_STAGE", REMOTE_STAGE),
            public_origin: env_or("SOW_PUBLIC_ORIGIN", PUBLIC_ORIGIN)
                .trim_end_matches('/')
                .to_string(),
            require_public: env::var("SOW_REQUIRE_PUBLIC").is_ok_and(|v| v == "1"),
        }
    }
}

struct Release {
    id: String,
    version: String,
    dir: PathBuf,
}

#[derive(Default, Debug)]
struct ComponentPlan {
    web: bool,
    maps: bool,
    server: bool,
    database: bool,
    ops: bool,
    relay: bool,
}

impl ComponentPlan {
    fn any(&self) -> bool {
        self.web || self.maps || self.server || self.database || self.ops || self.relay
    }
}

pub(super) fn execute(paths: &Paths, bump: bool) -> Result<()> {
    let config = Config::load();
    require_secret("SOW_DB_SECRET")?;
    require_secret("SOW_RELAY_CONTROL_SECRET")?;
    let version = version(paths, bump)?;

    println!("==> Production {version}");
    println!("==> 1/8 Preflight (read-only)");
    preflight(paths, &config)?;

    println!("==> 2/8 Build candidates");
    let (_web, backend, _relay) = std::thread::scope(|scope| {
        let web = scope.spawn(|| build_web(paths, &version));
        let backend = scope.spawn(|| build_freebsd(paths, &config));
        let relay = scope.spawn(|| build_relay(paths, &config));
        web.join()
            .map_err(|_| anyhow::anyhow!("web build panicked"))??;
        let backend = backend
            .join()
            .map_err(|_| anyhow::anyhow!("FreeBSD build panicked"))??;
        let relay = relay
            .join()
            .map_err(|_| anyhow::anyhow!("relay build panicked"))??;
        Ok::<_, anyhow::Error>(((), backend, relay))
    })?;

    println!("==> 3/8 Package immutable release");
    let release = assemble_release(paths, &paths.dist_web, &backend, &version, &config)?;
    println!("  release {}", release.id);

    println!("==> Runtime prerequisites");
    ensure_relay_runtime(&config)?;
    ensure_control_clock(&config)?;
    verify_control_runtime_secret(&config)?;
    verify_relay_control_path(&config)?;

    println!("==> 4/8 Compare deployed manifest");
    let mut plan = remote_plan(&config, &release)?;
    if maps_catalog_path_drift(&config)? {
        println!("  runtime env drift: SOW_MAPS_CATALOG_PATH");
        plan.ops = true;
    }
    println!("  plan: {plan:?}");

    if !plan.any() {
        println!("  no production component changed; no restart performed");
        verify_control_host(&config, &plan)?;
        verify_relay_runtime(&config)?;
        verify_control_runtime_secret(&config)?;
        verify_relay_control_path(&config)?;
        verify_relay_identity(&config, &release)?;
        retain_releases(&config)?;
        verify_public(paths, &config, &release)?;
        println!("✅ Production already serves the requested content");
        return Ok(());
    }

    println!("==> 5/8 Stage release (no service mutation)");
    stage_release(&config, &release)?;
    if plan.relay {
        stage_relay(&config, &release)?;
    }

    println!("==> 6/8 Activate changed components only");
    activate_control_host(paths, &config, &release, &plan)?;
    // Relay last: its worker swap force-kills active games (user-authorized
    // drain mode until a non-destructive drain exists), so the control host
    // must already be healthy before any relay worker is touched.
    if plan.relay {
        activate_relay_host(&config, &release)?;
    }

    println!("==> 7/8 Healthcheck and retain");
    verify_control_host(&config, &plan)?;
    verify_relay_runtime(&config)?;
    verify_control_runtime_secret(&config)?;
    verify_relay_control_path(&config)?;
    verify_relay_identity(&config, &release)?;
    retain_releases(&config)?;

    println!("==> 8/8 Public verification");
    verify_public(paths, &config, &release)?;
    println!("✅ Production {} ready as {}", release.version, release.id);
    Ok(())
}

fn require_secret(key: &str) -> Result<()> {
    let value =
        env::var(key).with_context(|| format!("{key} must be provided via sow-dist/.env"))?;
    if value.trim().is_empty() {
        bail!("{key} must not be empty");
    }
    Ok(())
}

fn env_or(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

fn env_or_alias(primary: &str, legacy: &str, default: &str) -> String {
    env::var(primary)
        .or_else(|_| env::var(legacy))
        .unwrap_or_else(|_| default.to_string())
}

fn version(paths: &Paths, bump: bool) -> Result<String> {
    let path = paths.root.join(".version");
    let current = fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?
        .trim()
        .to_string();
    if !bump {
        return Ok(current);
    }
    let mut parts = current
        .split('.')
        .map(str::parse::<u32>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .context(".version must be major.minor.patch")?;
    if parts.len() != 3 {
        bail!(".version must be major.minor.patch");
    }
    parts[2] += 1;
    let next = format!("{}.{}.{}", parts[0], parts[1], parts[2]);
    fs::write(&path, format!("{next}\n"))?;
    println!("  version {current} -> {next}");
    Ok(next)
}

fn preflight(paths: &Paths, config: &Config) -> Result<()> {
    for command in ["cargo", "curl", "rsync", "rustc", "scp", "ssh", "wasm-opt"] {
        if !Command::new("/bin/sh")
            .args(["-c", &format!("command -v {command} >/dev/null")])
            .status()?
            .success()
        {
            bail!("{command} is required");
        }
    }
    if !Command::new("rustc")
        .args([
            "--print",
            "target-libdir",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success()
    {
        bail!("Rust WASM standard library missing");
    }
    require_file(&paths.root.join("Cargo.toml"), "workspace Cargo.toml")?;
    run(
        "ssh",
        &[
            &config.control_host,
            "test -d /srv/sow/releases && command -v sudo >/dev/null",
        ],
        None,
    )
    .context("control host preflight failed")?;
    run(
        "ssh",
        &[
            &config.build_host,
            &format!(
                "test -d {} && command -v cargo >/dev/null",
                shell_quote(&config.build_root)
            ),
        ],
        None,
    )
    .context("FreeBSD builder preflight failed")?;
    run(
        "ssh",
        &[
            &config.relay_host,
            "command -v sudo >/dev/null && command -v systemctl >/dev/null && command -v curl >/dev/null",
        ],
        None,
    )
    .context("relay host preflight failed")
}

fn relay_worker_count() -> Result<usize> {
    let count = env_or("SOW_RELAY_WORKER_COUNT", "4")
        .parse::<usize>()
        .context("SOW_RELAY_WORKER_COUNT must be an integer")?;
    if !(1..=64).contains(&count) {
        bail!("SOW_RELAY_WORKER_COUNT must be between 1 and 64");
    }
    Ok(count)
}

fn relay_runtime_command(start_missing: bool) -> Result<String> {
    let count = relay_worker_count()?;
    let units = (0..count)
        .map(|id| format!("sow-relay@{id}.service"))
        .collect::<Vec<_>>();
    let mut command = String::from("set -eu; active=0;");
    for unit in &units {
        command.push_str(&format!(
            " if systemctl is-active --quiet {unit}; then active=$((active+1)); fi;"
        ));
    }
    if start_missing {
        // Recovery starts workers individually, never the aggregate unit:
        // `systemctl start sow-relay.service` historically left every worker
        // down (the group's Wants propagation does not recover stopped
        // instances), causing a full relay outage.
        let starts = (0..count)
            .map(|id| format!("timeout 90s sudo systemctl start sow-relay@{id}.service"))
            .collect::<Vec<_>>()
            .join("; ");
        command.push_str(&format!(
            " if [ \"$active\" -eq 0 ]; then sudo systemctl enable sow-relay.service >/dev/null; sudo systemctl daemon-reload >/dev/null; {starts}; elif [ \"$active\" -ne {count} ]; then echo 'partial relay worker failure; refusing unsafe DPDK recovery' >&2; exit 78; else sudo systemctl enable sow-relay.service >/dev/null; fi;"
        ));
    }
    command.push_str(" systemctl is-enabled --quiet sow-relay.service; systemctl is-active --quiet sow-relay.service; systemctl is-enabled --quiet chrony.service; systemctl is-active --quiet chrony.service; test \"$(timedatectl show -p NTPSynchronized --value)\" = yes;");
    for id in 0..count {
        let unit = format!("sow-relay@{id}.service");
        command.push_str(&format!(
            " test \"$(systemctl show {unit} -p ActiveState --value)\" = active; test \"$(systemctl show {unit} -p Result --value)\" = success; test \"$(systemctl show {unit} -p ExecMainStatus --value)\" = 0;"
        ));
        let port = 8080 + id;
        let retry = if start_missing {
            " --retry 10 --retry-connrefused --retry-delay 1"
        } else {
            ""
        };
        command.push_str(&format!(
            " curl -kfsS --max-time 5{retry} https://127.0.0.1:{port}/healthz >/dev/null;"
        ));
    }
    Ok(command)
}

fn ensure_relay_runtime(config: &Config) -> Result<()> {
    let command = relay_runtime_command(true)?;
    run("ssh", &[&config.relay_host, &command], None)
        .context("relay workers could not be made healthy")?;
    println!("  relay workers healthy");
    Ok(())
}

fn verify_relay_runtime(config: &Config) -> Result<()> {
    let command = relay_runtime_command(false)?;
    run("ssh", &[&config.relay_host, &command], None).context("relay worker healthcheck failed")
}

fn clock_skew_seconds(config: &Config) -> Result<u64> {
    let (control, relay) = std::thread::scope(|scope| {
        let control = scope.spawn(|| output("ssh", &[&config.control_host, "date -u +%s"]));
        let relay = scope.spawn(|| output("ssh", &[&config.relay_host, "date -u +%s"]));
        let control = control
            .join()
            .map_err(|_| anyhow::anyhow!("control clock check panicked"))??;
        let relay = relay
            .join()
            .map_err(|_| anyhow::anyhow!("relay clock check panicked"))??;
        Ok::<_, anyhow::Error>((control, relay))
    })?;
    let control = control
        .parse::<u64>()
        .context("control host returned an invalid UNIX timestamp")?;
    let relay = relay
        .parse::<u64>()
        .context("relay host returned an invalid UNIX timestamp")?;
    Ok(control.abs_diff(relay))
}

fn control_clock_configured(config: &Config) -> Result<bool> {
    let check = r#"cfg=/etc/rc.conf.d/ntpd; if sudo test -f "$cfg" && sudo grep -qx 'ntpd_enable="YES"' "$cfg" && sudo grep -qx 'ntpd_sync_on_start="YES"' "$cfg" && sudo service ntpd onestatus >/dev/null 2>&1; then echo ok; else echo drift; fi"#;
    Ok(output("ssh", &[&config.control_host, check])? == "ok")
}

fn ensure_control_clock(config: &Config) -> Result<()> {
    let configured = control_clock_configured(config)?;
    let skew = clock_skew_seconds(config)?;
    if !configured || skew > 5 {
        println!("  control clock drift detected ({skew}s); synchronizing through pipeline");
        let configure = r#"set -eu
cfg=/etc/rc.conf.d/ntpd
if ! sudo test -f "$cfg" || ! sudo grep -qx 'ntpd_enable="YES"' "$cfg" || ! sudo grep -qx 'ntpd_sync_on_start="YES"' "$cfg"; then
    if sudo test -e "$cfg"; then sudo cp -p "$cfg" "$cfg.bak_$(date +%s)"; fi
    tmp=$(mktemp /tmp/sow-ntpd.XXXXXX)
    trap 'rm -f "$tmp"' EXIT
    if sudo test -f "$cfg"; then sudo grep -v -E '^ntpd_(enable|sync_on_start)=' "$cfg" > "$tmp" || true; fi
    printf '%s\n' 'ntpd_enable="YES"' 'ntpd_sync_on_start="YES"' >> "$tmp"
    sudo install -o root -g wheel -m 0644 "$tmp" "$cfg"
fi
sudo service ntpd onestop >/dev/null 2>&1 || true
sudo /bin/timeout 60 /usr/sbin/ntpd -gq
sudo service ntpd onestart
sudo service ntpd onestatus >/dev/null"#;
        run("ssh", &[&config.control_host, configure], None)
            .context("control-host clock synchronization failed")?;
    }
    if !control_clock_configured(config)? {
        bail!("control-host NTP is not configured and running");
    }
    let skew = clock_skew_seconds(config)?;
    if skew > 5 {
        bail!("control/relay clock skew remains {skew}s after synchronization");
    }
    println!("  control/relay clock skew {skew}s");
    Ok(())
}

fn relay_management_workers() -> Result<Vec<(String, u16)>> {
    let raw = env::var("SOW_RELAY_WORKERS")
        .context("SOW_RELAY_WORKERS must configure every relay worker")?;
    let mut workers = Vec::new();
    for (id, spec) in raw.split(',').enumerate() {
        let fields = spec.trim().split(':').collect::<Vec<_>>();
        if fields.len() != 3 || fields[1].is_empty() {
            bail!("SOW_RELAY_WORKERS contains an invalid entry at index {id}");
        }
        let port = fields[2]
            .parse::<u16>()
            .with_context(|| format!("invalid relay management port at index {id}"))?;
        workers.push((fields[1].to_string(), port));
    }
    if workers.len() != relay_worker_count()? {
        bail!(
            "SOW_RELAY_WORKERS has {} entries but SOW_RELAY_WORKER_COUNT is {}",
            workers.len(),
            relay_worker_count()?
        );
    }
    Ok(workers)
}

fn verify_control_runtime_secret(config: &Config) -> Result<()> {
    let secret = env::var("SOW_RELAY_CONTROL_SECRET")?;
    let expected = format!("{:x}", Sha256::digest(secret.as_bytes()));
    let command = r#"set -eu
pid=$(sudo jexec sow-server pgrep -xo sow-server)
value=$(sudo procstat -e "$pid" | tr ' ' '\n' | sed -n 's/^SOW_RELAY_CONTROL_SECRET=//p')
test -n "$value"
printf %s "$value" | sha256 -q"#;
    let actual = output("ssh", &[&config.control_host, command])?;
    if actual != expected {
        bail!("running sow-server relay control secret does not match deployment secret");
    }
    Ok(())
}

/// Authenticated GET to every relay worker's management port, executed from
/// the control host (the only host the Azure NSG lets through). Returns one
/// response body per worker, in worker-id order.
fn relay_authed_get(config: &Config, path: &str) -> Result<Vec<(usize, String)>> {
    type HmacSha256 = Hmac<Sha256>;

    let secret = env::var("SOW_RELAY_CONTROL_SECRET")?;
    if env_or("SOW_RELAY_MGMT_SCHEME", "https") != "https" {
        bail!("SOW_RELAY_MGMT_SCHEME must be https in production");
    }
    let resolve_ip = env::var("SOW_RELAY_MGMT_RESOLVE_IP")
        .context("SOW_RELAY_MGMT_RESOLVE_IP is required for relay verification")?;
    let mut responses = Vec::new();
    for (worker_id, (host, port)) in relay_management_workers()?.into_iter().enumerate() {
        let timestamp = output("ssh", &[&config.control_host, "date -u +%s"])?;
        let mut nonce_bytes = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = hex::encode(nonce_bytes);
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|_| anyhow::anyhow!("invalid relay control secret"))?;
        mac.update(b"GET\n");
        mac.update(path.as_bytes());
        mac.update(b"\n");
        mac.update(timestamp.as_bytes());
        mac.update(b"\n");
        mac.update(nonce.as_bytes());
        mac.update(b"\n");
        let signature = hex::encode(mac.finalize().into_bytes());
        let url = format!("https://{host}:{port}{path}");
        let resolve = format!("{host}:{port}:{resolve_ip}");
        let command = format!(
            "curl -fsS --max-time 5 --resolve {} -H {} -H {} -H {} {}",
            shell_quote(&resolve),
            shell_quote(&format!("X-SOW-Timestamp: {timestamp}")),
            shell_quote(&format!("X-SOW-Nonce: {nonce}")),
            shell_quote(&format!("X-SOW-Signature: {signature}")),
            shell_quote(&url),
        );
        let response = output("ssh", &[&config.control_host, &command])
            .with_context(|| format!("authenticated relay probe failed for worker port {port}"))?;
        responses.push((worker_id, response));
    }
    Ok(responses)
}

/// Total live relay-player connections across all workers (drain report).
/// Informational only: the current drain mode is an authorized force-kill.
fn relay_active_connections(config: &Config) -> Result<usize> {
    let mut total = 0usize;
    for (worker_id, body) in relay_authed_get(config, "/internal/lobbies")? {
        let value: serde_json::Value = serde_json::from_str(&body)
            .with_context(|| format!("worker port {worker_id} returned invalid lobbies"))?;
        if let Some(lobbies) = value.get("lobbies").and_then(serde_json::Value::as_array) {
            for lobby in lobbies {
                if let Some(count) = lobby
                    .get("active_relay_connections")
                    .and_then(serde_json::Value::as_u64)
                {
                    total += count as usize;
                }
            }
        }
    }
    Ok(total)
}

fn verify_relay_control_path(config: &Config) -> Result<()> {
    let path = "/internal/metrics";
    let worker_count = relay_worker_count()?;
    for (worker_id, body) in relay_authed_get(config, path)? {
        let metrics: serde_json::Value = serde_json::from_str(&body)
            .with_context(|| format!("worker port {worker_id} returned invalid metrics"))?;
        if metrics.get("queue_id").and_then(serde_json::Value::as_u64) != Some(worker_id as u64)
            || metrics
                .get("queue_count")
                .and_then(serde_json::Value::as_u64)
                != Some(worker_count as u64)
        {
            bail!(
                "relay worker port {worker_id} reports queue_id/queue_count {:?}/{:?}, expected {worker_id}/{worker_count}",
                metrics.get("queue_id"),
                metrics.get("queue_count")
            );
        }
    }
    println!("  authenticated IONOS -> relay control path verified");
    Ok(())
}

fn stage_relay(config: &Config, release: &Release) -> Result<()> {
    let source = format!("{}/", release.dir.join("relay").display());
    let destination = format!("{}:{}", config.relay_host, RELAY_STAGE);
    run(
        "ssh",
        &[&config.relay_host, &format!("install -d -m 0700 {}", shell_quote(RELAY_STAGE))],
        None,
    )?;
    run(
        "rsync",
        &["-azc", "--delete", &source, &destination],
        Some(&release.dir),
    )?;
    Ok(())
}

/// Activate the relay component on the Azure host. Drain mode is an
/// authorized force-kill (user GO 2026-08-21: lobbies never drain on their
/// own, so workers are stopped and started individually — never the group
/// unit — and the kill is registered in the manifest).
fn activate_relay_host(config: &Config, release: &Release) -> Result<()> {
    let active = relay_active_connections(config)?;
    println!(
        "  relay drain: {active} active client connection(s) across workers — force-kill (registered)"
    );
    let env = RelayEnv::load()?;
    let revision = output("git", &["rev-parse", "--short=12", "HEAD"])?;
    let relay_fstack = fstack_version(config)?;
    let release_json: serde_json::Value =
        serde_json::from_slice(&fs::read(release.dir.join("release.json"))?)?;
    let relay_component = release_json
        .get("relay")
        .and_then(|relay| relay.get("sha256"))
        .and_then(serde_json::Value::as_str)
        .context("release.json relay sha256 missing")?;
    let relay_bin_sha = release_json
        .get("relay")
        .and_then(|relay| relay.get("bin_sha256"))
        .and_then(serde_json::Value::as_str)
        .context("release.json relay bin_sha256 missing")?;
    let db_secret = env::var("SOW_DB_SECRET")?;
    let control_secret = env::var("SOW_RELAY_CONTROL_SECRET")?;
    let db_path = format!("/tmp/sow-relay-db-secret-{}", std::process::id());
    let control_path = format!("/tmp/sow-relay-control-secret-{}", std::process::id());
    stage_secret(&config.relay_host, &db_secret, &db_path)?;
    stage_secret(&config.relay_host, &control_secret, &control_path)?;

    let ids = (0..env.count)
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let units = (0..env.count)
        .map(|id| format!("sow-relay@{id}.service"))
        .collect::<Vec<_>>()
        .join(" ");
    let remote = format!(
        r#"set -eu
stage={stage}
ts=$(date +%s)
for f in {exec} {config} /usr/local/sbin/sow-relay-worker /etc/systemd/system/sow-relay.service /etc/systemd/system/sow-relay@.service /etc/systemd/system/sow-relay@.service.d/override.conf /etc/tmpfiles.d/sow-relay.conf /etc/systemd/journald.conf.d/30-sow-relay.conf; do
  if sudo test -e "$f"; then sudo cp -p "$f" "$f.bak_$ts"; fi
done
if ! getent group {group} >/dev/null; then sudo groupadd --system {group}; fi
if ! id -u {user} >/dev/null 2>&1; then sudo useradd --system --gid {group} --home-dir /nonexistent --shell /usr/sbin/nologin {user}; fi
sudo install -d -o root -g {group} -m 0750 /usr/local/libexec/sow-relay /usr/local/etc/sow
sudo install -d -o {user} -g {group} -m 0750 {state} {replays}
sudo install -d -o {user} -g {group} -m 0700 /var/run/dpdk
sudo chown -R {user}:{group} /var/run/dpdk /dev/hugepages 2>/dev/null || true
sudo tee /etc/tmpfiles.d/sow-relay.conf >/dev/null <<'EOF'
d /var/run/dpdk 0700 sowrelay sowrelay -
Z /dev/hugepages 0700 sowrelay sowrelay -
EOF
sudo install -d -m 0755 /etc/systemd/journald.conf.d
sudo tee /etc/systemd/journald.conf.d/30-sow-relay.conf >/dev/null <<'EOF'
[Journal]
SystemMaxUse=256M
RuntimeMaxUse=128M
MaxRetentionSec=7day
RateLimitIntervalSec=30s
RateLimitBurst=10000
EOF
sudo install -o root -g {group} -m 0750 "$stage/bin/sow-relay" {exec}
sudo install -o root -g {group} -m 0640 "$stage/conf/echo-vf.ini" {config}
sudo install -o root -g root -m 0755 "$stage/ops/linux/sow-relay-worker" /usr/local/sbin/sow-relay-worker
sudo install -o root -g root -m 0644 "$stage/ops/linux/sow-relay.service" /etc/systemd/system/sow-relay.service
sudo install -o root -g root -m 0644 "$stage/ops/linux/sow-relay@.service" /etc/systemd/system/sow-relay@.service
sudo mkdir -p /etc/systemd/system/sow-relay@.service.d
db=$(cat {db_path}); ctl=$(cat {control_path}); rm -f {db_path} {control_path}
sed -e "s|__SOW_DB_SECRET__|$db|" -e "s|__SOW_RELAY_CONTROL_SECRET__|$ctl|" "$stage/ops/linux/sow-relay-override.conf.tmpl" | sudo tee /etc/systemd/system/sow-relay@.service.d/override.conf >/dev/null
sudo chmod 0600 /etc/systemd/system/sow-relay@.service.d/override.conf
sudo test -s /usr/local/etc/sow/relay.crt
sudo test -s /usr/local/etc/sow/relay.key
sudo openssl x509 -in /usr/local/etc/sow/relay.crt -noout -checkend 86400 >/dev/null
sudo systemctl daemon-reload
sudo systemctl enable sow-relay.service >/dev/null
for id in {ids}; do sudo systemctl stop "sow-relay@$id.service" 2>/dev/null || true; done
for id in {ids}; do
  sudo systemctl start "sow-relay@$id.service"
  port=$((8080+id))
  i=0
  until curl -kfsS --max-time 5 "https://127.0.0.1:$port/healthz" >/dev/null; do
    i=$((i+1))
    if [ "$i" -ge 60 ]; then echo "relay worker $id failed healthz after start" >&2; exit 78; fi
    sleep 1
  done
done
test "$(sudo stat -c '%a %U:%G' {exec})" = '750 root:{group}'
test "$(sudo stat -c '%a %U:%G' {config})" = '640 root:{group}'
test "$(sudo stat -c '%a' /etc/systemd/system/sow-relay@.service.d/override.conf)" = 600
for u in {units}; do
  test "$(systemctl show "$u" -p User --value)" = {user}
  test "$(systemctl show "$u" -p ActiveState --value)" = active
  test "$(systemctl show "$u" -p Result --value)" = success
  test "$(systemctl show "$u" -p ExecMainStatus --value)" = 0
done
sudo tee {manifest} >/dev/null <<EOF
{{"version":"{version}","release":"{release_id}","git":"{revision}","fstack":"{relay_fstack}","relay_sha256":"{relay_component}","relay_bin_sha256":"{relay_bin_sha}","ws_write_timeout_ms":{knob},"drain":"force-kill (user-authorized 2026-08-21; non-destructive drain pending)","deployed_at":"$ts"}}
EOF
sudo chown root:root {manifest}
sudo chmod 0644 {manifest}
"#,
        stage = RELAY_STAGE,
        exec = RELAY_EXEC,
        config = RELAY_CONFIG,
        group = RELAY_GROUP,
        user = RELAY_USER,
        state = RELAY_STATE,
        replays = RELAY_REPLAYS,
        manifest = RELAY_MANIFEST,
        db_path = db_path,
        control_path = control_path,
        ids = ids,
        units = units,
        version = release.version,
        release_id = release.id,
        revision = revision,
        relay_fstack = relay_fstack,
        relay_component = relay_component,
        relay_bin_sha = relay_bin_sha,
        knob = env.knob,
    );
    run("ssh", &[&config.relay_host, &remote], None)
        .context("relay activation failed")?;
    println!("  relay activated and registered ({})", &relay_component[..12]);
    Ok(())
}

/// The deployed relay must match the release exactly: manifest fields and the
/// [BOOT] identity line of the running worker. No manifest = the relay was
/// never deployed through the pipeline.
fn verify_relay_identity(config: &Config, release: &Release) -> Result<()> {
    let release_json: serde_json::Value =
        serde_json::from_slice(&fs::read(release.dir.join("release.json"))?)?;
    let relay = release_json.get("relay").context("release.json has no relay metadata")?;
    let expected_sha = relay
        .get("sha256")
        .and_then(serde_json::Value::as_str)
        .context("release relay sha256 missing")?;
    let expected_knob = relay
        .get("ws_write_timeout_ms")
        .and_then(serde_json::Value::as_u64)
        .context("release relay knob missing")?;
    let expected_git = release_json
        .get("git")
        .and_then(serde_json::Value::as_str)
        .context("release git missing")?;
    let expected_fstack = relay
        .get("fstack")
        .and_then(serde_json::Value::as_str)
        .context("release relay fstack missing")?;
    let expected_bin_sha = relay
        .get("bin_sha256")
        .and_then(serde_json::Value::as_str)
        .context("release relay bin_sha256 missing")?;
    let manifest = relay_manifest_remote(config)?
        .context("relay host has no registered manifest — relay never deployed through ./sow p")?;
    for (key, expected) in [
        ("relay_sha256", expected_sha),
        ("relay_bin_sha256", expected_bin_sha),
        ("git", expected_git),
        ("fstack", expected_fstack),
    ] {
        let actual = manifest.get(key).and_then(serde_json::Value::as_str).unwrap_or("");
        if actual != expected {
            bail!("relay manifest {key}={actual}, release expects {expected}");
        }
    }
    if manifest.get("ws_write_timeout_ms").and_then(serde_json::Value::as_u64) != Some(expected_knob)
    {
        bail!("relay manifest knob mismatch");
    }
    let boot = output(
        "ssh",
        &[&config.relay_host,
            "sudo journalctl -u sow-relay@0.service --no-pager -n 8000 2>/dev/null | grep '\\[BOOT\\] git=' | tail -1"],
    )?;
    if !boot.contains(&format!("git={expected_git}"))
        || !boot.contains(&format!("ws_write_timeout_ms={expected_knob}"))
    {
        bail!("relay worker 0 [BOOT] identity mismatch: {boot}");
    }
    // Content check: hash the binary the workers actually exec. Component
    // hashes and [BOOT] env stamps can be consistent while the deployed file
    // is stale (cargo not relinking against a rebuilt libfstack.a), so the
    // deployed file itself must match the release's recorded binary sha.
    let deployed = output(
        "ssh",
        &[&config.relay_host, &format!("sha256sum {RELAY_EXEC}")],
    )?;
    if !deployed.starts_with(expected_bin_sha) {
        bail!(
            "relay deployed binary mismatch: {deployed} expected {expected_bin_sha}"
        );
    }
    println!(
        "  relay identity verified (git={expected_git} fstack={expected_fstack} bin={expected_bin_sha} knob={expected_knob})"
    );
    Ok(())
}

fn build_web(paths: &Paths, version: &str) -> Result<()> {
    compile_wasm(paths, false)?;
    let fingerprint = input_fingerprint(
        "web-v6",
        version,
        &[
            &paths.wasm_input,
            &paths.shell,
            &paths.assets_cdn,
            &paths.assets_maps,
            &paths.assets_static,
            &paths.root.join("sow-i18n/src"),
            &paths.root.join("sow-i18n/strings"),
            &paths.root.join("sow-web/site"),
        ],
    )?;
    let cache = paths.root.join("dist/.sow-state/web-package");
    let cached = fs::read_to_string(&cache).is_ok_and(|value| value.trim() == fingerprint)
        && paths.dist_web.join("play/index.html").is_file()
        && paths.dist_cg.join("index.html").is_file()
        && verify_layout(&paths.dist_web).is_ok()
        && verify_cg_layout(&paths.dist_cg).is_ok();
    if cached {
        println!("==> Web package unchanged — reusing dist");
        return Ok(());
    }
    package_self(paths, &paths.dist_web, version)?;
    package_cg(&paths.dist_web, &paths.dist_cg, paths, version)?;
    fs::create_dir_all(cache.parent().context("web cache parent missing")?)?;
    fs::write(cache, format!("{fingerprint}\n"))?;
    Ok(())
}

fn build_freebsd(paths: &Paths, config: &Config) -> Result<PathBuf> {
    let local = paths.root.join("dist/freebsd-bin");
    let fingerprint = input_fingerprint(
        "freebsd-v3",
        "",
        &[
            &paths.root.join("Cargo.toml"),
            &paths.root.join("Cargo.lock"),
            &paths.root.join("sow-core"),
            &paths.root.join("sow-data"),
            &paths.root.join("sow-net"),
            &paths.root.join("sow-server"),
        ],
    )?;
    let cache = paths.root.join("dist/.sow-state/freebsd-build");
    if ["sow-server", "sow-database"]
        .iter()
        .all(|name| local.join(name).is_file())
        && fs::read_to_string(&cache).is_ok_and(|value| value.trim() == fingerprint)
    {
        println!("==> FreeBSD backend unchanged — reusing binaries");
        return Ok(local);
    }

    let source = format!("{}/", paths.root.display());
    let destination = format!("{}:{}/", config.build_host, config.build_root);
    run(
        "rsync",
        &[
            "-azc",
            "--delete",
            "--exclude=.git",
            "--exclude=dist",
            "--exclude=target",
            "--exclude=sow-dist/.env",
            &source,
            &destination,
        ],
        Some(&paths.root),
    )?;

    let root = shell_quote(&config.build_root);
    let command = format!(
        "set -eu; cd {root}; cargo test --locked -p sow-data --features server; cargo test --locked -p sow-server; cargo build --locked --profile deploy -p sow-server; cargo build --locked --profile deploy -p sow-data --features server --bin sow-database"
    );
    run("ssh", &[&config.build_host, &command], None)?;

    if local.exists() {
        fs::remove_dir_all(&local)?;
    }
    fs::create_dir_all(&local)?;
    for name in ["sow-server", "sow-database"] {
        let remote = format!(
            "{}:{}/target/deploy/{name}",
            config.build_host, config.build_root
        );
        let destination = local.join(name);
        run(
            "scp",
            &[
                &remote,
                destination.to_str().context("binary path is not UTF-8")?,
            ],
            None,
        )?;
        require_file(&destination, name)?;
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o550))?;
    }
    fs::create_dir_all(cache.parent().context("FreeBSD cache parent missing")?)?;
    fs::write(cache, format!("{fingerprint}\n"))?;
    Ok(local)
}

/// Registered WS write timeout (ms) for the relay. The pipeline stamps it into
/// the unit drop-in and the relay manifest, and the relay prints it in [BOOT].
fn relay_knob_ms() -> u64 {
    env_or("SOW_WS_WRITE_TIMEOUT_MS", "15000")
        .parse::<u64>()
        .unwrap_or(15000)
}

/// Relay runtime environment rendered into the unit drop-in. Every value here
/// is release content: it participates in the relay component hash, so a knob
/// or admission change re-deploys the relay instead of ghosting.
struct RelayEnv {
    count: usize,
    tickets_required: String,
    max_connections: String,
    max_connections_per_ip: String,
    handshakes_per_ip: String,
    db_url: String,
    db_resolve_ip: String,
    replay_spool: String,
    knob: u64,
}

impl RelayEnv {
    fn load() -> Result<Self> {
        let count = relay_worker_count()?;
        let tickets_required = env_or("SOW_RELAY_TICKETS_REQUIRED", "1");
        if tickets_required != "0" && tickets_required != "1" {
            bail!("SOW_RELAY_TICKETS_REQUIRED must be 0 or 1");
        }
        let max_connections = env_or("SOW_RELAY_MAX_CONNECTIONS", "32768");
        let max_connections_per_ip = env_or("SOW_RELAY_MAX_CONNECTIONS_PER_IP", "4096");
        let handshakes_per_ip = env_or("SOW_RELAY_HANDSHAKES_PER_IP", "512");
        let max_connections = max_connections
            .parse::<usize>()
            .context("SOW_RELAY_MAX_CONNECTIONS must be an integer")?;
        let max_connections_per_ip = max_connections_per_ip
            .parse::<usize>()
            .context("SOW_RELAY_MAX_CONNECTIONS_PER_IP must be an integer")?;
        let handshakes_per_ip = handshakes_per_ip
            .parse::<u32>()
            .context("SOW_RELAY_HANDSHAKES_PER_IP must be an integer")?;
        if max_connections == 0 || max_connections_per_ip == 0 || handshakes_per_ip == 0 {
            bail!("relay admission limits must be positive");
        }
        if max_connections_per_ip > max_connections {
            bail!("SOW_RELAY_MAX_CONNECTIONS_PER_IP must not exceed SOW_RELAY_MAX_CONNECTIONS");
        }
        let db_url = env_or("SOW_DB_URL", "https://shadowsofwar.io");
        if !db_url.starts_with("https://") {
            bail!("SOW_DB_URL must use https for relay production deploys");
        }
        let db_resolve_ip = env_or("SOW_DB_RESOLVE_IP", "74.208.246.177");
        db_resolve_ip
            .parse::<std::net::IpAddr>()
            .with_context(|| format!("invalid SOW_DB_RESOLVE_IP={db_resolve_ip}"))?;
        Ok(Self {
            count,
            tickets_required,
            max_connections: max_connections.to_string(),
            max_connections_per_ip: max_connections_per_ip.to_string(),
            handshakes_per_ip: handshakes_per_ip.to_string(),
            db_url,
            db_resolve_ip,
            replay_spool: RELAY_REPLAYS.to_string(),
            knob: relay_knob_ms(),
        })
    }
}

/// Registered identity of the f-stack tree used for the relay build.
fn fstack_version(config: &Config) -> Result<String> {
    let rev = output("git", &["-C", &config.fstack_repo, "rev-parse", "--short=12", "HEAD"])
        .with_context(|| format!("f-stack repo has no HEAD: {}", config.fstack_repo))?;
    let dirty = output("git", &["-C", &config.fstack_repo, "status", "--porcelain"])?;
    Ok(if dirty.is_empty() {
        rev
    } else {
        format!("{rev}-dirty")
    })
}

/// Build the relay on the Azure host (f-stack lib + binary) and fetch the
/// binary back into dist/relay-bin. Source of truth is always the repo: both
/// trees are rsynced with --delete before compiling.
fn build_relay(paths: &Paths, config: &Config) -> Result<PathBuf> {
    let fstack_hash = input_fingerprint("fstack-v1", "", &[Path::new(&config.fstack_repo)])?;
    let local = paths.root.join("dist/relay-bin");
    let fingerprint = input_fingerprint(
        "relay-v1",
        &fstack_hash,
        &[
            &paths.root.join("Cargo.toml"),
            &paths.root.join("Cargo.lock"),
            &paths.root.join("sow-relay"),
            &paths.root.join("fstack-bridge"),
            &paths.root.join("sow-dist/deploy/linux"),
            &paths.root.join("fstack-bridge/echo-vf.ini"),
        ],
    )?;
    let cache = paths.root.join("dist/.sow-state/relay-build");
    if local.join("sow-relay").is_file()
        && fs::read_to_string(&cache).is_ok_and(|value| value.trim() == fingerprint)
    {
        println!("==> Relay unchanged — reusing binary");
        return Ok(local);
    }

    println!("==> Building relay on {}", config.relay_host);
    let source = format!("{}/", paths.root.display());
    let destination = format!("{}:{}/", config.relay_host, config.relay_root);
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

    let fstack_source = format!("{}/", config.fstack_repo.trim_end_matches('/'));
    let fstack_destination = format!("{}:{}", config.relay_host, RELAY_FSTACK_ROOT);
    run(
        "rsync",
        &[
            "-azc",
            "--delete",
            "--exclude=.git",
            // Build artifacts (*.o/*.a) are gitignored local junk — never sync
            // them to the host, or make links objects compiled with a different
            // toolchain ("bad value" archive errors). dpdk/ is the DPDK 24.11.6
            // source tree (exact match for the host's installed librte
            // archives); it is synced so f-stack can compile against it.
            // dpdk/build is the meson build dir created on the host when
            // restoring the missing DPDK headers — never synced back or wiped,
            // or every deploy would trigger a full DPDK rebuild.
            "--exclude=*.o",
            "--exclude=*.a",
            "--exclude=dpdk/build",
            &fstack_source,
            &fstack_destination,
        ],
        Some(Path::new(&config.fstack_repo)),
    )?;

    let fstack_cache = paths.root.join("dist/.sow-state/fstack-build");
    // The cache key includes the make flags: the same tree can produce a lib
    // without the bridge's zc API if FF_ZC_RECV is dropped from the build.
    let fstack_key = format!("{fstack_hash}:FF_ZC_RECV=1");
    if !fs::read_to_string(&fstack_cache).is_ok_and(|value| value.trim() == fstack_key) {
        println!(
            "==> F-Stack changed ({}) — restoring DPDK headers + rebuilding libfstack.a on relay host",
            &fstack_hash[..12]
        );
        // f-stack compiles against DPDK headers installed at /usr/local/include
        // (rte_config.h and friends). The host lost that header tree while the
        // relay was outside the pipeline; restore it from the synced source
        // (version-matched with the installed librte archives) via the official
        // meson install, then build libfstack.a. FF_ZC_RECV=1 is required: the
        // bridge uses the zero-copy recv API (ff_zc_recv*), which compiles out
        // of the lib unless the make knob is set.
        let prepare = format!(
            "set -eu; export PATH=$HOME/.cargo/bin:$PATH; \
             if ! command -v meson >/dev/null 2>&1; then sudo DEBIAN_FRONTEND=noninteractive apt-get update -qq; sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq meson ninja-build python3-pyelftools; fi; \
             if ! python3 -c 'import elftools' >/dev/null 2>&1; then sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq python3-pyelftools; fi; \
             if ! test -f /usr/local/include/rte_config.h; then cd {dpdk_root}; if test -f build/build.ninja; then meson setup --reconfigure build -Dplatform=generic >/dev/null; else meson setup build -Dplatform=generic; fi; ninja -C build; sudo ninja -C build install; fi; \
             cd {fstack_root} && make -C lib clean >/dev/null 2>&1; make -C lib -j$(nproc) FF_ZC_RECV=1",
            dpdk_root = shell_quote(&format!("{RELAY_FSTACK_ROOT}/dpdk")),
            fstack_root = shell_quote(RELAY_FSTACK_ROOT)
        );
        run("ssh", &[&config.relay_host, &prepare], None)?;
        fs::write(&fstack_cache, format!("{fstack_key}\n"))?;
    }

    let build = format!(
        "set -eu; export PATH=$HOME/.cargo/bin:$PATH; export FSTACK_LIB_DIR={}; cd {} && cargo build --release -p sow-relay",
        shell_quote(&format!("{RELAY_FSTACK_ROOT}/lib")),
        shell_quote(&config.relay_root)
    );
    run("ssh", &[&config.relay_host, &build], None)?;

    if local.exists() {
        fs::remove_dir_all(&local)?;
    }
    fs::create_dir_all(&local)?;
    let remote = format!(
        "{}:{}/target/release/sow-relay",
        config.relay_host, config.relay_root
    );
    let destination = local.join("sow-relay");
    run(
        "scp",
        &[&remote, destination.to_str().context("relay path is not UTF-8")?],
        None,
    )?;
    require_file(&destination, "relay binary")?;
    fs::set_permissions(&destination, fs::Permissions::from_mode(0o550))?;
    fs::create_dir_all(cache.parent().context("relay cache parent missing")?)?;
    fs::write(cache, format!("{fingerprint}\n"))?;
    println!("  relay binary fetched ({} bytes)", fs::metadata(&destination)?.len());
    Ok(local)
}

fn input_fingerprint(tag: &str, value: &str, inputs: &[&Path]) -> Result<String> {
    let mut hash = Sha256::new();
    hash.update(tag.as_bytes());
    hash.update(value.as_bytes());
    for input in inputs {
        if input.is_file() {
            hash.update(input.to_string_lossy().as_bytes());
            hash.update(fs::read(input)?);
            continue;
        }
        if !input.is_dir() {
            hash.update(b"missing:");
            hash.update(input.to_string_lossy().as_bytes());
            continue;
        }
        let mut files = walkdir::WalkDir::new(input)
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        files.retain(|entry| entry.file_type().is_file());
        files.sort_by_key(|entry| entry.path().to_path_buf());
        for entry in files {
            hash.update(
                entry
                    .path()
                    .strip_prefix(input)?
                    .to_string_lossy()
                    .as_bytes(),
            );
            hash.update(fs::read(entry.path())?);
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn assemble_release(
    paths: &Paths,
    web: &Path,
    binaries: &Path,
    version: &str,
    config: &Config,
) -> Result<Release> {
    let revision = output("git", &["rev-parse", "--short=12", "HEAD"])?;
    let work = paths.root.join("dist/.release");
    if work.exists() {
        fs::remove_dir_all(&work)?;
    }
    fs::create_dir_all(work.join("bin"))?;
    copy_dir(web, &work.join("web"))?;
    let web_maps = work.join("web/maps");
    if web_maps.is_dir() {
        fs::rename(&web_maps, work.join("maps"))?;
    } else {
        copy_dir(&paths.assets_maps, &work.join("maps"))?;
    }
    for name in ["sow-server", "sow-database"] {
        let destination = work.join("bin").join(name);
        fs::copy(binaries.join(name), &destination)?;
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o550))?;
    }
    copy_dir(
        &paths.root.join("sow-dist/deploy/freebsd/rc.d"),
        &work.join("ops/rc.d"),
    )?;
    copy_dir(
        &paths.root.join("sow-dist/deploy/freebsd/conf.d"),
        &work.join("ops/conf.d"),
    )?;
    copy_dir(
        &paths.root.join("sow-dist/deploy/freebsd/snippets"),
        &work.join("ops/snippets"),
    )?;

    // Relay component: binary, config, systemd units, wrapper, and the rendered
    // drop-in template (secrets stay as placeholders, injected on the host).
    // Everything here is release content — the relay component hash changes
    // when the knob, admission limits, units, or binary change, so the plan
    // diff detects drift instead of ghosting.
    let relay = work.join("relay");
    fs::create_dir_all(relay.join("bin"))?;
    fs::create_dir_all(relay.join("conf"))?;
    fs::copy(
        paths.root.join("dist/relay-bin/sow-relay"),
        relay.join("bin/sow-relay"),
    )?;
    fs::set_permissions(relay.join("bin/sow-relay"), fs::Permissions::from_mode(0o550))?;
    fs::copy(
        paths.root.join("fstack-bridge/echo-vf.ini"),
        relay.join("conf/echo-vf.ini"),
    )?;
    let ops = relay.join("ops/linux");
    fs::create_dir_all(&ops)?;
    let env = RelayEnv::load()?;
    let wants = (0..env.count)
        .map(|id| format!("sow-relay@{id}.service"))
        .collect::<Vec<_>>()
        .join(" ");
    let stops = (0..env.count)
        .rev()
        .map(|id| format!("sow-relay@{id}.service"))
        .collect::<Vec<_>>()
        .join(" ");
    let secondary_ids = if env.count > 1 {
        (1..env.count)
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join("|")
    } else {
        String::new()
    };
    for (name, tokens) in [
        ("sow-relay.service", &[("__WANTS__", wants.as_str()), ("__STOPS__", stops.as_str())][..]),
        ("sow-relay-worker", &[("__SECONDARY_IDS__", secondary_ids.as_str())][..]),
        ("sow-relay@.service", &[][..]),
    ] {
        let src = paths
            .root
            .join("sow-dist/deploy/linux")
            .join(format!("{name}.tmpl"));
        let mut content = fs::read_to_string(&src)?;
        for (token, value) in tokens {
            content = content.replace(token, value);
        }
        fs::write(ops.join(name), content)?;
    }
    let mut override_tpl =
        fs::read_to_string(paths.root.join("sow-dist/deploy/linux/sow-relay-override.conf.tmpl"))?;
    let relay_fstack = fstack_version(config)?;
    for (token, value) in [
        ("__SOW_RELAY_WORKER_COUNT__", env.count.to_string()),
        ("__SOW_RELAY_TICKETS_REQUIRED__", env.tickets_required.clone()),
        ("__SOW_RELAY_MAX_CONNECTIONS__", env.max_connections.clone()),
        ("__SOW_RELAY_MAX_CONNECTIONS_PER_IP__", env.max_connections_per_ip.clone()),
        ("__SOW_RELAY_HANDSHAKES_PER_IP__", env.handshakes_per_ip.clone()),
        ("__SOW_DB_URL__", env.db_url.clone()),
        ("__SOW_DB_RESOLVE_IP__", env.db_resolve_ip.clone()),
        ("__SOW_REPLAY_SPOOL_DIR__", env.replay_spool.clone()),
        ("__SOW_WS_WRITE_TIMEOUT_MS__", env.knob.to_string()),
        ("__SOW_RELAY_GIT__", revision.clone()),
        ("__SOW_FSTACK_VERSION__", relay_fstack.clone()),
    ] {
        override_tpl = override_tpl.replace(token, &value);
    }
    // The two secret placeholders are injected on the host (sed) — they are
    // the only tokens allowed to survive this check.
    let secrets_only = override_tpl
        .replace("__SOW_DB_SECRET__", "")
        .replace("__SOW_RELAY_CONTROL_SECRET__", "");
    if secrets_only.contains("__SOW_") {
        bail!("relay override template has an unrendered token");
    }
    fs::write(ops.join("sow-relay-override.conf.tmpl"), override_tpl)?;
    require_file(&relay.join("bin/sow-relay"), "relay binary")?;
    require_file(&relay.join("conf/echo-vf.ini"), "relay config")?;

    let nginx_site = fs::read_to_string(
        paths
            .root
            .join("sow-dist/deploy/freebsd/conf.d/shadowsofwar.io.conf"),
    )?;
    let relay_ip = env_or("SOW_RELAY_DB_SOURCE_IP", "20.230.49.9");
    let nginx_site = nginx_site
        .replace("__SOW_RELAY_DB_SOURCE_IP__", &relay_ip)
        .replace("__SOW_DB_LISTEN_HOST__", DATABASE_JAIL_IP)
        .replace("__SOW_SERVER_LISTEN_HOST__", SERVER_JAIL_IP)
        .replace("__SOW_MAPS_LISTEN_HOST__", SERVER_JAIL_IP);
    if nginx_site.contains("__SOW_") {
        bail!("nginx placeholder was not rendered");
    }
    fs::write(work.join("ops/conf.d/shadowsofwar.io.conf"), nginx_site)?;

    require_file(&work.join("web/index.html"), "website index")?;
    require_file(&work.join("web/play/index.html"), "game index")?;
    require_file(&work.join("web/robots.txt"), "robots.txt")?;
    require_file(&work.join("web/sitemap.xml"), "sitemap.xml")?;
    require_file(&work.join("web/game-manifest.json"), "game manifest")?;
    require_file(&work.join("maps/world/map.bin"), "server map")?;

    let components = [
        ("web", component_hash(&work.join("web"))?),
        ("maps", component_hash(&work.join("maps"))?),
        ("server", component_hash(&work.join("bin/sow-server"))?),
        ("database", component_hash(&work.join("bin/sow-database"))?),
        ("ops", component_hash(&work.join("ops"))?),
        ("relay", component_hash(&work.join("relay"))?),
    ];
    let component_text = components
        .iter()
        .map(|(name, hash)| format!("{name}={hash}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(work.join("COMPONENTS"), format!("{component_text}\n"))?;
    fs::write(work.join("VERSION"), format!("{version}\n"))?;
    let relay_component = components
        .iter()
        .find(|(name, _)| *name == "relay")
        .map(|(_, hash)| hash.clone())
        .context("relay component missing")?;
    let relay_bin_sha = file_sha256(&work.join("relay/bin/sow-relay"))?;
    fs::write(
        work.join("release.json"),
        serde_json::to_vec_pretty(&json!({
            "version": version,
            "git": revision,
            "components": components.iter().map(|(name, hash)| json!({"name": name, "sha256": hash})).collect::<Vec<_>>(),
            "relay": json!({
                "sha256": relay_component,
                "bin_sha256": relay_bin_sha,
                "fstack": relay_fstack,
                "ws_write_timeout_ms": env.knob,
                "drain": "force-kill (user-authorized 2026-08-21; non-destructive drain pending)",
            }),
        }))?,
    )?;
    let manifest = write_manifest(&work)?;
    let id = format!("{version}-{}", &file_sha256(&manifest)?[..12]);
    let releases = paths.root.join("dist/releases");
    fs::create_dir_all(&releases)?;
    let dir = releases.join(&id);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::rename(&work, &dir)?;
    Ok(Release {
        id,
        version: version.to_string(),
        dir,
    })
}

fn component_hash(path: &Path) -> Result<String> {
    input_fingerprint("component-v1", "", &[path])
}

fn write_manifest(root: &Path) -> Result<PathBuf> {
    let mut files = walkdir::WalkDir::new(root)
        .into_iter()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    files.retain(|entry| entry.file_type().is_file() && entry.file_name() != "SHA256");
    files.sort_by_key(|entry| entry.path().to_path_buf());
    let mut contents = String::new();
    for entry in files {
        let relative = entry
            .path()
            .strip_prefix(root)?
            .to_str()
            .context("release path is not UTF-8")?;
        contents.push_str(&format!("{}  {relative}\n", file_sha256(entry.path())?));
    }
    let path = root.join("SHA256");
    fs::write(&path, contents)?;
    Ok(path)
}

/// The relay host's registered deploy manifest (written by activate_relay_host).
fn relay_manifest_remote(config: &Config) -> Result<Option<serde_json::Value>> {
    let raw = output(
        "ssh",
        &[&config.relay_host, &format!("sudo cat {RELAY_MANIFEST} 2>/dev/null || true")],
    )?;
    if raw.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&raw)
        .map(Some)
        .context("relay host manifest is not valid JSON")
}

fn remote_plan(config: &Config, release: &Release) -> Result<ComponentPlan> {
    let remote = output(
        "ssh",
        &[
            &config.control_host,
            "if test -f /srv/sow/current/COMPONENTS; then cat /srv/sow/current/COMPONENTS; fi",
        ],
    )?;
    let current = parse_components(&remote);
    let local = parse_components(&fs::read_to_string(release.dir.join("COMPONENTS"))?);
    let relay_remote = relay_manifest_remote(config)?
        .and_then(|manifest| {
            manifest
                .get("relay_sha256")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        });
    Ok(ComponentPlan {
        web: current.get("web") != local.get("web"),
        maps: current.get("maps") != local.get("maps"),
        server: current.get("server") != local.get("server"),
        database: current.get("database") != local.get("database"),
        ops: current.get("ops") != local.get("ops"),
        relay: relay_remote != local.get("relay").cloned(),
    })
}

fn maps_catalog_path_drift(config: &Config) -> Result<bool> {
    let expected = env_or("SOW_MAPS_CATALOG_PATH", "/var/db/sow/server/catalog.bin");
    let remote = output(
        "ssh",
        &[
            &config.control_host,
            r#"for f in /usr/local/etc/sow/sow.env /zroot/jails/sow-server/usr/local/etc/sow/sow.env /zroot/jails/sow-database/usr/local/etc/sow/sow.env; do if sudo test -f "$f"; then sudo awk -F= '$1=="SOW_MAPS_CATALOG_PATH"{print $2}' "$f"; fi; done"#,
        ],
    )?;
    let values = remote.lines().map(str::trim).collect::<Vec<_>>();
    Ok(values.len() != 3 || values.iter().any(|value| *value != expected))
}

fn parse_components(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .collect()
}

fn stage_release(config: &Config, release: &Release) -> Result<()> {
    let stage = format!("{}/release", config.remote_stage.trim_end_matches('/'));
    let prepare = format!(
        "install -d -m 0700 {0} {0}/bin {0}/maps {0}/ops {0}/web",
        shell_quote(&stage)
    );
    run("ssh", &[&config.control_host, &prepare], None)?;
    let source = format!("{}/", release.dir.display());
    let destination = format!("{}:{}/", config.control_host, stage);
    run(
        "rsync",
        &["-azc", "--delete", &source, &destination],
        Some(&release.dir),
    )
}

fn activate_control_host(
    paths: &Paths,
    config: &Config,
    release: &Release,
    plan: &ComponentPlan,
) -> Result<()> {
    let stage = format!("{}/release", config.remote_stage.trim_end_matches('/'));
    let target = format!("{REMOTE_RELEASES}/{}", release.id);
    let server_restart = plan.server || plan.ops;
    let db_restart = plan.database || plan.ops;
    let nginx_reload = plan.ops;
    let runtime_env = plan.server || plan.database || plan.ops;
    let (db_secret_file, control_secret_file) = if runtime_env {
        let db_secret = env::var("SOW_DB_SECRET")?;
        let control_secret = env::var("SOW_RELAY_CONTROL_SECRET")?;
        let db_path = format!("/tmp/sow-db-secret-{}", std::process::id());
        let control_path = format!("/tmp/sow-relay-control-secret-{}", std::process::id());
        stage_secret(&config.control_host, &db_secret, &db_path)?;
        stage_secret(&config.control_host, &control_secret, &control_path)?;
        (Some(db_path), Some(control_path))
    } else {
        (None, None)
    };
    let relay_host = env_or("SOW_RELAY_HOST", "relay.shadowsofwar.io");
    let relay_workers = env::var("SOW_RELAY_WORKERS").unwrap_or_default();
    let relay_worker_count = env_or("SOW_RELAY_WORKER_COUNT", "4");
    let relay_mgmt_url = env_or("SOW_RELAY_MGMT_URL", "https://127.0.0.1:8080");
    let relay_mgmt_scheme = env_or("SOW_RELAY_MGMT_SCHEME", "https");
    let relay_mgmt_resolve_ip = env_or("SOW_RELAY_MGMT_RESOLVE_IP", "20.230.49.9");
    let relay_tickets_required = env_or("SOW_RELAY_TICKETS_REQUIRED", "1");
    let maps_root = env_or("SOW_MAPS_ROOT", "/srv/sow/current/maps");
    let maps_catalog_path = env_or("SOW_MAPS_CATALOG_PATH", "/var/db/sow/server/catalog.bin");
    let env_update = if runtime_env {
        format!(
            "mkdir -p /tmp/sow-env-update; for f in /usr/local/etc/sow/sow.env /zroot/jails/sow-server/usr/local/etc/sow/sow.env /zroot/jails/sow-database/usr/local/etc/sow/sow.env; do t=$(mktemp /tmp/sow.env.XXXXXX); if sudo test -f \"$f\"; then sudo grep -v -E '^(SOW_MAPS_ROOT|SOW_MAPS_CATALOG_PATH|SOW_DB_SECRET|SOW_RELAY_CONTROL_SECRET|SOW_RELAY_HOST|SOW_RELAY_WORKERS|SOW_RELAY_WORKER_COUNT|SOW_RELAY_MGMT_URL|SOW_RELAY_MGMT_SCHEME|SOW_RELAY_MGMT_RESOLVE_IP|SOW_RELAY_TICKETS_REQUIRED)=|^[0-9a-fA-F]{{64}}$' \"$f\" > \"$t\" || true; else : > \"$t\"; fi; printf '%s\\n' SOW_MAPS_ROOT={maps_root} SOW_MAPS_CATALOG_PATH={maps_catalog_path} SOW_RELAY_HOST={relay_host} SOW_RELAY_WORKERS={relay_workers} SOW_RELAY_WORKER_COUNT={relay_worker_count} SOW_RELAY_MGMT_URL={relay_mgmt_url} SOW_RELAY_MGMT_SCHEME={relay_mgmt_scheme} SOW_RELAY_MGMT_RESOLVE_IP={relay_mgmt_resolve_ip} SOW_RELAY_TICKETS_REQUIRED={relay_tickets_required} | sudo tee -a \"$t\" >/dev/null; printf '%s' 'SOW_DB_SECRET=' | sudo tee -a \"$t\" >/dev/null; sudo cat {db_secret} | sudo tee -a \"$t\" >/dev/null; printf '\\n' | sudo tee -a \"$t\" >/dev/null; printf '%s' 'SOW_RELAY_CONTROL_SECRET=' | sudo tee -a \"$t\" >/dev/null; sudo cat {control_secret} | sudo tee -a \"$t\" >/dev/null; printf '\\n' | sudo tee -a \"$t\" >/dev/null; sudo install -o root -g wheel -m 0600 \"$t\" \"$f\"; rm -f \"$t\"; done; rm -rf /tmp/sow-env-update",
            maps_root = shell_quote(&maps_root),
            maps_catalog_path = shell_quote(&maps_catalog_path),
            relay_host = shell_quote(&relay_host),
            relay_workers = shell_quote(&relay_workers),
            relay_worker_count = shell_quote(&relay_worker_count),
            relay_mgmt_url = shell_quote(&relay_mgmt_url),
            relay_mgmt_scheme = shell_quote(&relay_mgmt_scheme),
            relay_mgmt_resolve_ip = shell_quote(&relay_mgmt_resolve_ip),
            relay_tickets_required = shell_quote(&relay_tickets_required),
            db_secret = shell_quote(db_secret_file.as_deref().unwrap()),
            control_secret = shell_quote(control_secret_file.as_deref().unwrap()),
        )
    } else {
        ":".to_string()
    };
    let secret_cleanup = if runtime_env {
        format!(
            "rm -f {} {}",
            shell_quote(db_secret_file.as_deref().unwrap()),
            shell_quote(control_secret_file.as_deref().unwrap())
        )
    } else {
        ":".to_string()
    };
    let mut remote = String::from(
        r#"set -eu
old=$(sudo readlink /srv/sow/current 2>/dev/null || true)
target=__TARGET__
stage=__STAGE__
rollback() {
    set +e
    sudo rm -f /srv/sow/current
    if [ -n "$old" ]; then sudo ln -s "$old" /srv/sow/current; fi
    if __DB_RESTART__; then sudo jexec sow-database service sow_database restart || true; fi
    if __SERVER_RESTART__; then sudo jexec sow-server service sow_server restart || true; fi
    __SECRET_CLEANUP__
    exit 78
}
__SECRET_TRAP__
sudo install -d -m 0755 __RELEASES__
sudo rm -rf "$target"
sudo mkdir -p "$target"
sudo cp -Rp "$stage/." "$target/"
sudo chown -R root:sow "$target"
sudo find "$target" -type d -exec chmod 0755 {} +
sudo find "$target" -type f -exec chmod 0644 {} +
sudo chmod 0550 "$target"/bin/*
sudo install -o root -g wheel -m 0555 "$target/ops/rc.d/sow_server" /zroot/jails/sow-server/usr/local/etc/rc.d/sow_server
sudo install -o root -g wheel -m 0555 "$target/ops/rc.d/sow_database" /zroot/jails/sow-database/usr/local/etc/rc.d/sow_database
link="/srv/sow/.current.$$"
sudo ln -s "releases/__ID__" "$link"
sudo mv -hf "$link" /srv/sow/current
if __NGINX_RELOAD__; then
    for f in "$target"/ops/conf.d/*; do [ -f "$f" ] || continue; sudo install -o root -g wheel -m 0644 "$f" "/usr/local/etc/nginx/conf.d/$(basename "$f")"; done
    for f in "$target"/ops/snippets/*; do [ -f "$f" ] || continue; sudo install -o root -g wheel -m 0644 "$f" "/usr/local/etc/nginx/snippets/$(basename "$f")"; done
    sudo nginx -t || rollback
    sudo service nginx reload || rollback
fi
__ENV_UPDATE__
if __DB_RESTART__; then sudo jexec sow-database service sow_database restart || rollback; fi
if __SERVER_RESTART__; then sudo jexec sow-server service sow_server restart || rollback; fi
if ! sudo jexec sow-database service sow_database status >/dev/null 2>&1; then rollback; fi
if ! sudo jexec sow-server service sow_server status >/dev/null 2>&1; then rollback; fi
__SECRET_CLEANUP__
"#,
    );
    let replacements = [
        ("__TARGET__", shell_quote(&target)),
        ("__STAGE__", shell_quote(&stage)),
        ("__RELEASES__", shell_quote(REMOTE_RELEASES)),
        ("__ID__", release.id.clone()),
        (
            "__DB_RESTART__",
            if db_restart {
                "true".to_string()
            } else {
                "false".to_string()
            },
        ),
        (
            "__SERVER_RESTART__",
            if server_restart {
                "true".to_string()
            } else {
                "false".to_string()
            },
        ),
        (
            "__NGINX_RELOAD__",
            if nginx_reload {
                "true".to_string()
            } else {
                "false".to_string()
            },
        ),
        ("__ENV_UPDATE__", env_update),
        (
            "__SECRET_TRAP__",
            if runtime_env {
                format!(
                    "trap 'rm -f {} {}' EXIT",
                    shell_quote(db_secret_file.as_deref().unwrap()),
                    shell_quote(control_secret_file.as_deref().unwrap())
                )
            } else {
                ":".to_string()
            },
        ),
        ("__SECRET_CLEANUP__", secret_cleanup),
    ];
    for (token, value) in replacements {
        remote = remote.replace(token, &value);
    }
    run("ssh", &[&config.control_host, &remote], Some(&paths.root))
        .context("granular control-host activation failed")
}

fn stage_secret(host: &str, secret: &str, remote_path: &str) -> Result<()> {
    let mut file = tempfile::NamedTempFile::new().context("create secret staging file")?;
    file.write_all(secret.as_bytes())?;
    file.as_file().sync_all()?;
    fs::set_permissions(file.path(), fs::Permissions::from_mode(0o600))?;
    let local_path = file.path().to_str().context("secret path is not UTF-8")?;
    run(
        "scp",
        &["-q", local_path, &format!("{host}:{remote_path}")],
        None,
    )
}

fn verify_control_host(config: &Config, plan: &ComponentPlan) -> Result<()> {
    let mut checks = String::from(
        "set -eu; sudo jexec sow-database service sow_database status >/dev/null; sudo jexec sow-server service sow_server status >/dev/null; i=0; until curl -fsS --max-time 5 http://127.0.0.1:25585/healthz >/dev/null; do i=$((i+1)); [ \"$i\" -ge 180 ] && exit 1; sleep 1; done; /usr/local/bin/valkey-cli -h 127.0.0.1 ping | grep -q PONG; j=0; until sudo sockstat -4l | grep -q '127.0.0.1:25564'; do j=$((j+1)); [ \"$j\" -ge 180 ] && exit 1; sleep 1; done",
    );
    if plan.web || plan.ops {
        checks.push_str("; sudo nginx -t");
    }
    run("ssh", &[&config.control_host, &checks], None).context("control-host healthcheck failed")
}

fn retain_releases(config: &Config) -> Result<()> {
    let command = format!(
        "set -eu; current=$(sudo readlink /srv/sow/current 2>/dev/null || true); case \"$current\" in releases/*) current_path=\"/srv/sow/$current\" ;; *) current_path=\"$current\" ;; esac; i=0; for dir in $(sudo ls -dt {REMOTE_RELEASES}/* 2>/dev/null || true); do i=$((i+1)); [ $i -le 5 ] && continue; [ \"$current_path\" = \"$dir\" ] && continue; case \"$dir\" in {REMOTE_RELEASES}/*) sudo rm -rf -- \"$dir\" ;; esac; done; sudo find {REMOTE_RELEASES} -maxdepth 2 -type l -name '.current.*' -delete"
    );
    run("ssh", &[&config.control_host, &command], None).context("release retention failed")
}

fn verify_public(paths: &Paths, config: &Config, release: &Release) -> Result<()> {
    let local_manifest = fs::read_to_string(release.dir.join("web/game-manifest.json"))?;
    let url = format!("{}/game-manifest.json", config.public_origin);
    match output("curl", &["-fsS", "--max-time", "20", &url]) {
        Ok(manifest) if manifest.trim() == local_manifest.trim() => {
            for url in [
                format!("{}/", config.public_origin),
                format!("{}/play/", config.public_origin),
            ] {
                run(
                    "curl",
                    &["-fsS", "--max-time", "20", "-o", "/dev/null", &url],
                    Some(&paths.root),
                )
                .with_context(|| format!("public origin check failed for {url}"))?;
            }
            println!("  public origin verified for {}", release.id);
            Ok(())
        }
        Ok(_) | Err(_) if !config.require_public => {
            println!("  public verification skipped (SOW_REQUIRE_PUBLIC != 1)");
            Ok(())
        }
        Ok(_) => bail!("public domain does not serve {}", release.id),
        Err(error) => Err(error).context("public verification failed"),
    }
}
