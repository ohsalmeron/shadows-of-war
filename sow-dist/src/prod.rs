use super::*;
use serde_json::json;

const BUILD_HOST: &str = "freebsd";
const BUILD_ROOT: &str = "/home/bizkit/shadows-of-war";
const PROD_HOST: &str = "ionos";
const REMOTE_STAGE: &str = "/home/bizkit/.sow-deploy";
const PUBLIC_ORIGIN: &str = "https://shadowsofwar.io";
const DEFAULT_RELAY_DB_SOURCE_IP: &str = "20.230.49.9";

struct Config {
    build_host: String,
    build_root: String,
    prod_host: String,
    remote_stage: String,
    public_origin: String,
    require_public: bool,
}

impl Config {
    fn load() -> Self {
        Self {
            build_host: env_or("SOW_BUILD_HOST", BUILD_HOST),
            build_root: env_or("SOW_BUILD_ROOT", BUILD_ROOT),
            prod_host: env_or("SOW_PROD_HOST", PROD_HOST),
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

pub(super) fn execute(paths: &Paths, bump: bool) -> Result<()> {
    let config = Config::load();
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
    let version = version(paths, bump)?;
    let relay_db_source_ip = relay_db_source_ip()?;

    println!("==> Production {version}");
    println!("==> 1/6 Preflight");
    preflight(&config)?;
    reject_untracked_source_files(paths)?;

    println!("==> 2/6 Web + FreeBSD backend (parallel)");
    let binaries = std::thread::scope(|scope| {
        let web = scope.spawn(|| build_web(paths, &version));
        let backend = scope.spawn(|| build_freebsd(paths, &config));

        let web = web
            .join()
            .map_err(|_| anyhow::anyhow!("web build panicked"))?;
        let backend = backend
            .join()
            .map_err(|_| anyhow::anyhow!("FreeBSD build panicked"))?;
        web?;
        backend
    })?;

    println!("==> 3/7 F-Stack relay");
    relay::execute(paths).context("F-Stack relay deployment failed")?;

    println!("==> 4/7 Release");
    let release = assemble_release(paths, &binaries, &version, &relay_db_source_ip)?;
    println!("  {}", release.id);

    println!("==> 5/7 Upload");
    sync_relay_env(&config)?;
    sync_prod_secrets(&config, &db_secret, &control_secret)?;
    deploy(paths, &config, &release)?;

    println!("==> 6/7 Origin verified by activator");
    println!("==> 7/7 Public verification");
    verify_public(paths, &config, &release)?;

    println!("✅ Production {} ready as {}", release.version, release.id);
    Ok(())
}

/// Refuse to copy untracked files into build or relay worktrees. Production
/// releases may be built from tracked-but-dirty source during development, but
/// an untracked artifact has no reviewable or reproducible provenance.
fn reject_untracked_source_files(paths: &Paths) -> Result<()> {
    let status = Command::new("git")
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .current_dir(&paths.root)
        .output()
        .context("inspect source tree hygiene")?;
    if !status.status.success() {
        bail!(
            "git status failed while checking source hygiene: {}",
            String::from_utf8_lossy(&status.stderr).trim()
        );
    }
    let status_text = String::from_utf8_lossy(&status.stdout);
    let untracked: Vec<String> = status_text
        .lines()
        .filter(|line| line.starts_with("?? "))
        .map(|line| line[3..].to_string())
        .collect();
    if !untracked.is_empty() {
        bail!(
            "untracked source files are not allowed in ./sow p:\n{}\nCommit or remove them before deploying",
            untracked.join("\n")
        );
    }
    Ok(())
}

fn sync_prod_secrets(config: &Config, db_secret: &str, control_secret: &str) -> Result<()> {
    let remote_secret = format!("/tmp/sow-db-secret-{}", std::process::id());
    let remote_control_secret = format!("/tmp/sow-relay-control-secret-{}", std::process::id());
    stage_secret(&config.prod_host, db_secret, &remote_secret)?;
    stage_secret(&config.prod_host, control_secret, &remote_control_secret)?;
    let remote = format!(
        "set -eu; secret_file={}; control_file={}; f=/usr/local/etc/sow/sow.env; t=$(mktemp /tmp/sow.env.XXXXXX); \\
         trap 'rm -f \"$t\" \"$secret_file\" \"$control_file\"' EXIT; \\
         chmod 600 \"$secret_file\"; \\
         for pid in $(sudo ps -axo pid= -o command= | awk '$2 == \"daemon:\" && index($0, \"/root/shadowsofwar/sow-database\") {{print $1}}'); do sudo kill -TERM \"$pid\"; done; \\
         sleep 1; \\
         for pid in $(sudo ps -axo pid= -o command= | awk '$2 == \"/root/shadowsofwar/sow-database\" {{print $1}}'); do sudo kill -TERM \"$pid\"; done; \\
         if sudo test -f \"$f\"; then sudo grep -v '^SOW_DB_SECRET=' \"$f\" > \"$t\"; else : > \"$t\"; fi; \\
         printf 'SOW_DB_SECRET=' >> \"$t\"; cat \"$secret_file\" >> \"$t\"; printf '\\n' >> \"$t\"; \\
         printf 'SOW_RELAY_CONTROL_SECRET=' >> \"$t\"; cat \"$control_file\" >> \"$t\"; printf '\\n' >> \"$t\"; \\
         sudo install -o root -g wheel -m 0600 \"$t\" \"$f\"; \\
         sudo service sow_database restart; sudo service sow_database status",
        shell_quote(&remote_secret),
        shell_quote(&remote_control_secret)
    );
    run("ssh", &[&config.prod_host, &remote], None).context("production secret sync failed")
}

fn env_or(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

fn relay_db_source_ip() -> Result<String> {
    let value = env_or("SOW_RELAY_DB_SOURCE_IP", DEFAULT_RELAY_DB_SOURCE_IP);
    value
        .parse::<std::net::IpAddr>()
        .with_context(|| format!("invalid SOW_RELAY_DB_SOURCE_IP={value}"))?;
    Ok(value)
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

fn preflight(config: &Config) -> Result<()> {
    for command in ["cargo", "curl", "rsync", "rustc", "scp", "ssh", "wasm-opt"] {
        let check = format!("command -v {command} >/dev/null");
        if !Command::new("/bin/sh")
            .args(["-c", &check])
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
        bail!("Rust WASM standard library missing (Arch package: rust-wasm)");
    }

    let worker_catalog = env::var("SOW_RELAY_WORKERS")
        .context("SOW_RELAY_WORKERS must configure the dynamic-routing workers")?;
    let configured_count = env::var("SOW_RELAY_WORKER_COUNT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|count| (1..=64).contains(count))
        .unwrap_or(4);
    let catalog_count = worker_catalog
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .count();
    if catalog_count != configured_count {
        bail!(
            "SOW_RELAY_WORKERS must contain exactly {configured_count} workers (found {catalog_count})"
        );
    }

    run(
        "ssh",
        &[
            &config.prod_host,
            "test -d /srv/sow/releases && command -v sudo >/dev/null && command -v sha256sum >/dev/null",
        ],
        None,
    )
    .context("FreeBSD production VM is not ready")
}

fn sync_relay_env(config: &Config) -> Result<()> {
    let relay_host = env::var("SOW_RELAY_HOST")
        .context("SOW_RELAY_HOST must identify the relay data-plane address")?;
    let workers = env::var("SOW_RELAY_WORKERS")
        .context("SOW_RELAY_WORKERS must configure the dynamic-routing workers")?;
    let worker_count = env::var("SOW_RELAY_WORKER_COUNT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|count| (1..=64).contains(count))
        .unwrap_or_else(|| workers.split(',').filter(|s| !s.trim().is_empty()).count());
    let mgmt_scheme = env_or("SOW_RELAY_MGMT_SCHEME", "https").to_ascii_lowercase();
    if mgmt_scheme != "https" {
        bail!("SOW_RELAY_MGMT_SCHEME must be https for production deploys");
    }
    let tickets_required = env_or("SOW_RELAY_TICKETS_REQUIRED", "1");
    if tickets_required != "0" && tickets_required != "1" {
        bail!("SOW_RELAY_TICKETS_REQUIRED must be 0 or 1");
    }
    let mgmt_resolve_ip = env::var("SOW_RELAY_MGMT_RESOLVE_IP")
        .context("SOW_RELAY_MGMT_RESOLVE_IP must identify the relay management NIC")?;
    mgmt_resolve_ip
        .parse::<std::net::IpAddr>()
        .with_context(|| format!("invalid SOW_RELAY_MGMT_RESOLVE_IP={mgmt_resolve_ip}"))?;
    let mgmt_url = env::var("SOW_RELAY_MGMT_URL")
        .unwrap_or_else(|_| "https://127.0.0.1:8080".to_string());
    let parsed_mgmt_url = url::Url::parse(&mgmt_url)
        .with_context(|| format!("invalid SOW_RELAY_MGMT_URL={mgmt_url}"))?;
    if parsed_mgmt_url.scheme() != "https" {
        bail!("SOW_RELAY_MGMT_URL must use https for production deploys");
    }
    let remote = format!(
        "set -eu; f=/usr/local/etc/sow/sow.env; t=$(mktemp /tmp/sow.env.XXXXXX); \\
         if sudo test -f \"$f\"; then sudo grep -v -E '^(SOW_RELAY_HOST|SOW_RELAY_WORKER_COUNT|SOW_RELAY_WORKERS|SOW_RELAY_PORT|SOW_RELAY_MGMT_URL|SOW_RELAY_MGMT_SCHEME|SOW_RELAY_MGMT_RESOLVE_IP|SOW_RELAY_TICKETS_REQUIRED)=' \"$f\" > \"$t\"; else : > \"$t\"; fi; \\
         printf '%s\\n' SOW_RELAY_HOST={} SOW_RELAY_WORKER_COUNT={} SOW_RELAY_WORKERS={} SOW_RELAY_PORT=80 SOW_RELAY_MGMT_URL={} SOW_RELAY_MGMT_SCHEME={} SOW_RELAY_MGMT_RESOLVE_IP={} SOW_RELAY_TICKETS_REQUIRED={} >> \"$t\"; \\
         sudo install -o root -g wheel -m 0600 \"$t\" \"$f\"; rm -f \"$t\"; sudo service sow_server restart; sudo service sow_server status",
        shell_quote(&relay_host),
        worker_count,
        shell_quote(&workers),
        shell_quote(&mgmt_url),
        shell_quote(&mgmt_scheme),
        shell_quote(&mgmt_resolve_ip),
        shell_quote(&tickets_required),
    );
    run("ssh", &[&config.prod_host, &remote], None).context("relay catalog sync failed")
}

fn build_web(paths: &Paths, version: &str) -> Result<()> {
    compile_wasm(paths, false)?;
    let fingerprint = input_fingerprint(
        "web-v2",
        version,
        &[
            &paths.wasm_input,
            &paths.shell,
            &paths.assets_cdn,
            &paths.assets_maps,
            &paths.assets_static,
            &paths.root.join("sow-i18n/src"),
            &paths.root.join("sow-i18n/strings"),
            &paths.root.join("sow-server/src/admin_dashboard.html"),
            &paths.root.join("sow-dist/src/main.rs"),
        ],
    )?;
    let cache = paths.root.join("dist/.sow-state/web-package");
    let cached = fs::read_to_string(&cache).is_ok_and(|value| value.trim() == fingerprint)
        && paths.dist_play.join("play/index.html").is_file()
        && paths.dist_cg.join("play/index.html").is_file()
        && verify_layout(&paths.dist_play).is_ok();

    if cached {
        println!("==> Web package unchanged — reusing dist");
        return Ok(());
    }

    package_self(paths, &paths.dist_play, version)?;
    package_cg(&paths.dist_play, &paths.dist_cg, paths)?;
    fs::create_dir_all(cache.parent().context("web cache parent missing")?)?;
    fs::write(cache, format!("{fingerprint}\n"))?;
    Ok(())
}

fn build_freebsd(paths: &Paths, config: &Config) -> Result<PathBuf> {
    let local = paths.root.join("dist/freebsd-bin");
    let fingerprint = input_fingerprint(
        "freebsd-v2",
        "",
        &[
            &paths.root.join("Cargo.toml"),
            &paths.root.join("Cargo.lock"),
            &paths.root.join("sow-core"),
            &paths.root.join("sow-data"),
            &paths.root.join("sow-net"),
            &paths.root.join("sow-relay"),
            &paths.root.join("sow-server"),
        ],
    )?;
    let cache = paths.root.join("dist/.sow-state/freebsd-build");
    let binaries_ready = ["sow-server", "sow-database"]
        .iter()
        .all(|name| local.join(name).is_file());

    if binaries_ready && fs::read_to_string(&cache).is_ok_and(|value| value.trim() == fingerprint) {
        let cleanup = format!(
            "set -eu; rm -f {root}/scripts/audit-connections.sh {root}/docs/connection-audit-contract.md; rmdir {root}/scripts 2>/dev/null || true",
            root = shell_quote(&config.build_root),
        );
        run("ssh", &[&config.build_host, &cleanup], None)
            .context("clean stale untracked audit artifacts on FreeBSD build VM")?;
        println!("==> FreeBSD backend unchanged — reusing binaries");
        return Ok(local);
    }

    let build_check = format!(
        "test -d {} && command -v cargo >/dev/null",
        shell_quote(&config.build_root)
    );
    run("ssh", &[&config.build_host, &build_check], None)
        .context("FreeBSD build VM is not ready")?;

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
        "set -eu; cd {root}; \
         cargo test --locked -p sow-data --features server; \
         cargo test --locked -p sow-server; \
         cargo build --locked --profile deploy -p sow-server; \
         cargo build --locked --profile deploy -p sow-data --features server --bin sow-database"
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
        // scp cannot overwrite the previous content-addressed artifact after
        // its mode is tightened to 0550. Remove it explicitly so a rebuild
        // remains deterministic even if a prior run was interrupted between
        // the directory cleanup and the copy loop.
        if destination.exists() {
            fs::remove_file(&destination)
                .with_context(|| format!("remove stale local binary {}", destination.display()))?;
        }
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
    binaries: &Path,
    version: &str,
    relay_db_source_ip: &str,
) -> Result<Release> {
    let revision = output("git", &["rev-parse", "--short=12", "HEAD"])?;
    let dirty = !output("git", &["status", "--porcelain", "--untracked-files=no"])?.is_empty();
    let fingerprint = input_fingerprint(
        "release-v2",
        &format!("{version}:{revision}:{dirty}:{relay_db_source_ip}"),
        &[
            &paths.dist_play,
            binaries,
            &paths.assets_maps,
            &paths.root.join("sow-dist/deploy/freebsd/rc.d"),
            &paths.root.join("sow-dist/deploy/freebsd/conf.d"),
            &paths.root.join("sow-dist/deploy/freebsd/nginx.conf"),
        ],
    )?;
    let cache = paths.root.join("dist/.sow-state/release");
    if let Ok(value) = fs::read_to_string(&cache) {
        if let Some((cached_fingerprint, id)) = value.trim().split_once(' ') {
            let dir = paths.root.join("dist/releases").join(id);
            if cached_fingerprint == fingerprint && dir.join("SHA256").is_file() {
                println!("==> Release unchanged — reusing {id}");
                return Ok(Release {
                    id: id.to_string(),
                    version: version.to_string(),
                    dir,
                });
            }
        }
    }

    let work = paths.root.join("dist/.release");
    if work.exists() {
        fs::remove_dir_all(&work)?;
    }
    fs::create_dir_all(work.join("bin"))?;

    copy_dir(&paths.dist_play, &work.join("web"))?;
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
    fs::copy(
        paths.root.join("sow-dist/deploy/freebsd/nginx.conf"),
        work.join("ops/nginx.conf"),
    )?;
    let nginx_site = fs::read_to_string(
        paths
            .root
            .join("sow-dist/deploy/freebsd/conf.d/shadowsofwar.io.conf"),
    )?
    .replace("__SOW_RELAY_DB_SOURCE_IP__", relay_db_source_ip);
    if nginx_site.contains("__SOW_RELAY_DB_SOURCE_IP__") {
        bail!("nginx relay source IP placeholder was not rendered");
    }
    fs::write(work.join("ops/conf.d/shadowsofwar.io.conf"), nginx_site)?;

    require_file(&work.join("web/play/index.html"), "web index")?;
    require_file(&work.join("web/game-manifest.json"), "game manifest")?;
    require_file(&work.join("maps/world/map.bin"), "server map")?;

    fs::write(work.join("VERSION"), format!("{version}\n"))?;
    fs::write(
        work.join("release.json"),
        serde_json::to_vec_pretty(&json!({
            "version": version,
            "git": revision,
            "dirty": dirty,
            "platform": "FreeBSD amd64"
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
    fs::create_dir_all(cache.parent().context("release cache parent missing")?)?;
    fs::write(cache, format!("{fingerprint} {id}\n"))?;

    Ok(Release {
        id,
        version: version.to_string(),
        dir,
    })
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

fn deploy(paths: &Paths, config: &Config, release: &Release) -> Result<()> {
    let stage_release = format!("{}/release", config.remote_stage.trim_end_matches('/'));
    let prepare = format!("install -d -m 0700 {}", shell_quote(&stage_release));
    run("ssh", &[&config.prod_host, &prepare], None)?;

    let source = format!("{}/", release.dir.display());
    let destination = format!("{}:{stage_release}/", config.prod_host);
    run(
        "rsync",
        &["-azc", "--delete", &source, &destination],
        Some(&paths.root),
    )?;

    let id = &release.id;
    let target = format!("/srv/sow/releases/{id}");
    let remote_cmd = format!(
        "set -eu; \
         sudo install -d -m 0755 /srv/sow/releases; \
         sudo service sow_server stop 2>/dev/null || true; \
         sudo service sow_database stop 2>/dev/null || true; \
         sudo mkdir -p \"{target}\"; \
         sudo cp -Rp \"{stage_release}/.\" \"{target}/\"; \
         sudo chown -R root:sow \"{target}\"; \
         sudo find \"{target}\" -type d -exec chmod 0755 {{}} +; \
         sudo find \"{target}\" -type f -exec chmod 0644 {{}} +; \
         sudo chmod 0550 \"{target}\"/bin/*; \
         if [ -d \"{target}/maps\" ]; then sudo chmod -R 0777 \"{target}/maps\"; fi; \
         link=\"/srv/sow/.current.$$\"; \
         sudo ln -s \"releases/{id}\" \"$link\"; \
         sudo mv -fh \"$link\" /srv/sow/current; \
         sudo install -d -m 0755 /usr/local/etc/nginx/conf.d; \
         if sudo test -f \"{target}/ops/conf.d/shadowsofwar.io.conf\"; then \
             sudo install -o root -g wheel -m 0644 \"{target}/ops/conf.d/shadowsofwar.io.conf\" /usr/local/etc/nginx/conf.d/shadowsofwar.io.conf; \
         fi; \
         if sudo test -f \"{target}/ops/nginx.conf\"; then \
             if ! sudo grep -q \"conf.d\" /usr/local/etc/nginx/nginx.conf 2>/dev/null; then \
                 sudo install -o root -g wheel -m 0644 \"{target}/ops/nginx.conf\" /usr/local/etc/nginx/nginx.conf; \
             fi; \
         fi; \
         sudo service sow_database start; \
         sudo service sow_database status; \
         sudo service sow_server restart; \
         sudo nginx -t && sudo service nginx reload"
    );

    run("ssh", &[&config.prod_host, &remote_cmd], None).context("native Rust deployment activation failed")
}

fn verify_public(paths: &Paths, config: &Config, release: &Release) -> Result<()> {
    let local_manifest = fs::read_to_string(release.dir.join("web/game-manifest.json"))?;
    let url = format!("{}/game-manifest.json", config.public_origin);
    let public = output("curl", &["-fsS", "--max-time", "20", &url]);

    match public {
        Ok(manifest) if manifest.trim() == local_manifest.trim() => {
            let play = format!("{}/play/", config.public_origin);
            run(
                "curl",
                &["-fsS", "--max-time", "20", "-o", "/dev/null", &play],
                Some(&paths.root),
            )?;
            println!("  public domain serves {}", release.id);
            Ok(())
        }
        Ok(_) | Err(_) if !config.require_public => {
            println!(
                "  public domain is not on {}; Azure origin passed",
                release.id
            );
            Ok(())
        }
        Ok(_) => bail!("public domain does not serve {}", release.id),
        Err(error) => Err(error).context("public verification failed"),
    }
}
