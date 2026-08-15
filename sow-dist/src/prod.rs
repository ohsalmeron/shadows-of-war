use super::*;
use serde_json::json;
use std::collections::BTreeMap;

const BUILD_HOST: &str = "freebsd";
const BUILD_ROOT: &str = "/home/bizkit/shadows-of-war";
const CONTROL_HOST: &str = "ionos";
const REMOTE_STAGE: &str = "/home/bizkit/.sow-deploy";
const REMOTE_RELEASES: &str = "/srv/sow/releases";
const PUBLIC_ORIGIN: &str = "https://shadowsofwar.io";
const SERVER_JAIL_IP: &str = "127.0.0.1";
const DATABASE_JAIL_IP: &str = "127.0.0.1";

struct Config {
    build_host: String,
    build_root: String,
    control_host: String,
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
}

impl ComponentPlan {
    fn any(&self) -> bool {
        self.web || self.maps || self.server || self.database || self.ops
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
    let (_web, backend) = std::thread::scope(|scope| {
        let web = scope.spawn(|| build_web(paths, &version));
        let backend = scope.spawn(|| build_freebsd(paths, &config));
        web.join()
            .map_err(|_| anyhow::anyhow!("web build panicked"))??;
        let backend = backend
            .join()
            .map_err(|_| anyhow::anyhow!("FreeBSD build panicked"))??;
        Ok::<_, anyhow::Error>(((), backend))
    })?;

    println!("==> 3/8 Package immutable release");
    let release = assemble_release(paths, &paths.dist_play, &backend, &version)?;
    println!("  release {}", release.id);

    println!("==> 4/8 Compare deployed manifest");
    let plan = remote_plan(&config, &release)?;
    println!("  plan: {plan:?}");
    println!("  relay: held; pipeline has no safe drain contract yet");

    if !plan.any() {
        println!("  no production component changed; no restart performed");
        verify_public(paths, &config, &release)?;
        println!("✅ Production already serves the requested content");
        return Ok(());
    }

    println!("==> 5/8 Stage release (no service mutation)");
    stage_release(&config, &release)?;

    println!("==> 6/8 Activate changed components only");
    activate_control_host(paths, &config, &release, &plan)?;

    println!("==> 7/8 Healthcheck and retain");
    verify_control_host(&config, &plan)?;
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
    .context("FreeBSD builder preflight failed")
}

fn build_web(paths: &Paths, version: &str) -> Result<()> {
    compile_wasm(paths, false)?;
    let fingerprint = input_fingerprint(
        "web-v3",
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

fn assemble_release(paths: &Paths, web: &Path, binaries: &Path, version: &str) -> Result<Release> {
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

    require_file(&work.join("web/play/index.html"), "web index")?;
    require_file(&work.join("web/game-manifest.json"), "game manifest")?;
    require_file(&work.join("maps/world/map.bin"), "server map")?;

    let components = [
        ("web", component_hash(&work.join("web"))?),
        ("maps", component_hash(&work.join("maps"))?),
        ("server", component_hash(&work.join("bin/sow-server"))?),
        ("database", component_hash(&work.join("bin/sow-database"))?),
        ("ops", component_hash(&work.join("ops"))?),
    ];
    let component_text = components
        .iter()
        .map(|(name, hash)| format!("{name}={hash}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(work.join("COMPONENTS"), format!("{component_text}\n"))?;
    fs::write(work.join("VERSION"), format!("{version}\n"))?;
    fs::write(
        work.join("release.json"),
        serde_json::to_vec_pretty(&json!({
            "version": version,
            "git": revision,
            "components": components.iter().map(|(name, hash)| json!({"name": name, "sha256": hash})).collect::<Vec<_>>(),
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
    Ok(ComponentPlan {
        web: current.get("web") != local.get("web"),
        maps: current.get("maps") != local.get("maps"),
        server: current.get("server") != local.get("server"),
        database: current.get("database") != local.get("database"),
        ops: current.get("ops") != local.get("ops"),
    })
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
    let env_update = if runtime_env {
        format!(
            "mkdir -p /tmp/sow-env-update; for f in /usr/local/etc/sow/sow.env /zroot/jails/sow-server/usr/local/etc/sow/sow.env /zroot/jails/sow-database/usr/local/etc/sow/sow.env; do t=$(mktemp /tmp/sow.env.XXXXXX); if sudo test -f \"$f\"; then sudo grep -v -E '^(SOW_DB_SECRET|SOW_RELAY_CONTROL_SECRET|SOW_RELAY_HOST|SOW_RELAY_WORKERS|SOW_RELAY_WORKER_COUNT|SOW_RELAY_MGMT_URL|SOW_RELAY_MGMT_SCHEME|SOW_RELAY_MGMT_RESOLVE_IP|SOW_RELAY_TICKETS_REQUIRED)=' \"$f\" > \"$t\" || true; else : > \"$t\"; fi; printf '%s\\n' SOW_RELAY_HOST={relay_host} SOW_RELAY_WORKERS={relay_workers} SOW_RELAY_WORKER_COUNT={relay_worker_count} SOW_RELAY_MGMT_URL={relay_mgmt_url} SOW_RELAY_MGMT_SCHEME={relay_mgmt_scheme} SOW_RELAY_MGMT_RESOLVE_IP={relay_mgmt_resolve_ip} SOW_RELAY_TICKETS_REQUIRED={relay_tickets_required} | sudo tee -a \"$t\" >/dev/null; printf '%s' 'SOW_DB_SECRET=' | sudo tee -a \"$t\" >/dev/null; sudo cat {db_secret} | sudo tee -a \"$t\" >/dev/null; printf '%s\\n' 'SOW_RELAY_CONTROL_SECRET=' | sudo tee -a \"$t\" >/dev/null; sudo cat {control_secret} | sudo tee -a \"$t\" >/dev/null; sudo install -o root -g wheel -m 0600 \"$t\" \"$f\"; rm -f \"$t\"; done; rm -rf /tmp/sow-env-update",
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
sudo mv -f "$link" /srv/sow/current
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
        "set -eu; sudo jexec sow-database service sow_database status >/dev/null; sudo jexec sow-server service sow_server status >/dev/null; curl -fsS --max-time 5 http://127.0.0.1:25585/healthz >/dev/null; /usr/local/bin/valkey-cli -h 127.0.0.1 ping | grep -q PONG; sudo sockstat -4l | grep -q '127.0.0.1:25564'",
    );
    if plan.web || plan.ops {
        checks.push_str("; sudo nginx -t");
    }
    run("ssh", &[&config.control_host, &checks], None).context("control-host healthcheck failed")
}

fn retain_releases(config: &Config) -> Result<()> {
    let command = format!(
        "set -eu; current=$(sudo readlink /srv/sow/current 2>/dev/null || true); case \"$current\" in releases/*) current_path=\"/srv/sow/$current\" ;; *) current_path=\"$current\" ;; esac; i=0; for dir in $(sudo ls -dt {REMOTE_RELEASES}/* 2>/dev/null || true); do i=$((i+1)); [ $i -le 5 ] && continue; [ \"$current_path\" = \"$dir\" ] && continue; case \"$dir\" in {REMOTE_RELEASES}/*) sudo rm -rf -- \"$dir\" ;; esac; done"
    );
    run("ssh", &[&config.control_host, &command], None).context("release retention failed")
}

fn verify_public(paths: &Paths, config: &Config, release: &Release) -> Result<()> {
    let local_manifest = fs::read_to_string(release.dir.join("web/game-manifest.json"))?;
    let url = format!("{}/game-manifest.json", config.public_origin);
    match output("curl", &["-fsS", "--max-time", "20", &url]) {
        Ok(manifest) if manifest.trim() == local_manifest.trim() => {
            run(
                "curl",
                &[
                    "-fsS",
                    "--max-time",
                    "20",
                    "-o",
                    "/dev/null",
                    &format!("{}/play/", config.public_origin),
                ],
                Some(&paths.root),
            )?;
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
