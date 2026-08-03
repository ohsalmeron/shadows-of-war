use super::*;
use serde_json::json;

const BUILD_HOST: &str = "freebsd";
const BUILD_ROOT: &str = "/home/bizkit/shadows-of-war";
const PROD_HOST: &str = "ionos";
const REMOTE_STAGE: &str = "/home/bizkit/.sow-deploy";
const PUBLIC_ORIGIN: &str = "http://74.208.246.177";

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
    let version = version(paths, bump)?;

    println!("==> Production {version}");
    println!("==> 1/6 Preflight");
    preflight(&config)?;

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

    println!("==> 3/6 Release");
    let release = assemble_release(paths, &binaries, &version)?;
    println!("  {}", release.id);

    println!("==> 4/6 Upload");
    deploy(paths, &config, &release)?;

    println!("==> 5/6 Origin verified by activator");
    println!("==> 6/6 Public verification");
    verify_public(paths, &config, &release)?;

    println!("✅ Production {} ready as {}", release.version, release.id);
    Ok(())
}

fn env_or(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
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

fn assemble_release(paths: &Paths, binaries: &Path, version: &str) -> Result<Release> {
    let revision = output("git", &["rev-parse", "--short=12", "HEAD"])?;
    let dirty = !output("git", &["status", "--porcelain", "--untracked-files=no"])?.is_empty();
    let fingerprint = input_fingerprint(
        "release-v2",
        &format!("{version}:{revision}:{dirty}"),
        &[
            &paths.dist_play,
            binaries,
            &paths.root.join("sow-dist/deploy/freebsd/rc.d"),
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
    fs::copy(
        paths.root.join("sow-dist/deploy/freebsd/nginx.conf"),
        work.join("ops/nginx.conf"),
    )?;

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

    let activate = paths
        .root
        .join("sow-dist/deploy/freebsd/activate-release.sh");
    require_file(&activate, "activation script")?;
    let remote_activate = format!(
        "{}:{}/activate-release.sh",
        config.prod_host,
        config.remote_stage.trim_end_matches('/')
    );
    run(
        "scp",
        &[
            activate.to_str().context("activation path is not UTF-8")?,
            &remote_activate,
        ],
        None,
    )?;

    let script = format!(
        "{}/activate-release.sh",
        config.remote_stage.trim_end_matches('/')
    );
    run(
        "ssh",
        &[
            &config.prod_host,
            "sudo",
            "/bin/sh",
            &script,
            &release.id,
            &release.version,
            &stage_release,
        ],
        None,
    )
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
