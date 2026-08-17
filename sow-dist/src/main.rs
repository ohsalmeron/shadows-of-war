use anyhow::{Context, Result, bail};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{env, fs};

mod prod;

const WASM_OPT_TAG: &str = "oz-cli-v1";

fn run(cmd: &str, args: &[&str], cwd: Option<&Path>) -> Result<()> {
    println!(
        "+ {cmd} {}",
        args.iter()
            .map(|a| shell_quote(a))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let mut c = Command::new(cmd);
    c.args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(d) = cwd {
        c.current_dir(d);
    }
    if !c.spawn()?.wait()?.success() {
        bail!("{cmd} failed");
    }
    Ok(())
}

fn output(cmd: &str, args: &[&str]) -> Result<String> {
    let o = Command::new(cmd).args(args).output()?;
    if !o.status.success() {
        bail!("{cmd} failed: {}", String::from_utf8_lossy(&o.stderr));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "./_=-+:,".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

struct Paths {
    root: PathBuf,
    shell: PathBuf,
    assets_cdn: PathBuf,
    assets_maps: PathBuf,
    assets_static: PathBuf,
    dist_web: PathBuf,
    dist_cg: PathBuf,
    wasm_input: PathBuf,
    wasm_cache: PathBuf,
}

impl Paths {
    fn discover() -> Result<Self> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()?;
        let t = env::var("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| root.join("target"));
        let s = root.join("dist/.sow-state");
        Ok(Self {
            wasm_input: t.join("wasm32-unknown-unknown/wasm-release/sow_client.wasm"),
            wasm_cache: s.join("wasm-opt-cache"),
            shell: root.join("sow-web/shell"),
            assets_cdn: root.join("assets/cdn"),
            assets_maps: root.join("assets/maps"),
            assets_static: root.join("assets/static"),
            dist_web: root.join("dist/web"),
            dist_cg: root.join("dist/crazygames"),
            root,
        })
    }
}

fn file_sha256(path: &Path) -> Result<String> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut b = [0u8; 65536];
    loop {
        let n = f.read(&mut b)?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for e in fs::read_dir(src)? {
        let e = e?;
        let to = dst.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            fs::copy(e.path(), to)?;
        }
    }
    Ok(())
}

fn require_file(path: &Path, label: &str) -> Result<()> {
    if !path.is_file() {
        bail!("{label} missing: {}", path.display());
    }
    if fs::metadata(path)?.len() == 0 {
        bail!("{label} empty: {}", path.display());
    }
    Ok(())
}

fn brotli_dst(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.br", path.display()))
}

fn compress_brotli(src: &Path, dst: &Path) -> Result<()> {
    let input = fs::read(src)?;
    if input.is_empty() {
        bail!("brotli source empty");
    }
    let mut out = Vec::new();
    let mut w = brotli::CompressorWriter::new(&mut out, 4096, 9, 22);
    w.write_all(&input)?;
    w.flush()?;
    drop(w);
    fs::write(dst, &out)?;
    Ok(())
}

fn prune_qs(root: &Path) -> Result<()> {
    let mut n = 0u32;
    for e in walkdir::WalkDir::new(root) {
        let e = e?;
        if e.file_type().is_file() && e.file_name().to_string_lossy().contains("?v=") {
            fs::remove_file(e.path())?;
            n += 1;
        }
    }
    if n > 0 {
        println!("Removed {n} querystring artifact(s)");
    }
    Ok(())
}

fn compile_wasm(paths: &Paths, dev: bool) -> Result<()> {
    println!("==> Compiling WASM (wasm-release)...");
    let mut a = vec![
        "build",
        "--profile",
        "wasm-release",
        "-p",
        "sow-client",
        "--target",
        "wasm32-unknown-unknown",
    ];
    if dev {
        a.extend_from_slice(&["--features", "dev"]);
        println!("==> (local) dev tools enabled");
    }
    let mut c = Command::new("cargo");
    c.args(&a)
        .current_dir(&paths.root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    c.env("RUSTFLAGS", "-C target-feature=-bulk-memory");
    if !c.spawn()?.wait()?.success() {
        bail!("WASM compile failed");
    }
    require_file(&paths.wasm_input, "WASM output")?;
    Ok(())
}

fn run_bindgen(wasm: &Path, out: &Path, name: &str) -> Result<()> {
    println!("==> Running wasm-bindgen for {name}...");
    wasm_bindgen_cli_support::Bindgen::new()
        .input_path(wasm)
        .web(true)?
        .out_name(name)
        .typescript(false)
        .generate(out)?;
    println!("✅ wasm-bindgen finished");
    Ok(())
}

fn run_wasm_opt(path: &Path, cache: &Path) -> Result<()> {
    require_file(path, "wasm-opt input")?;
    let hash = file_sha256(path)?;
    fs::create_dir_all(cache)?;
    let c = cache.join(format!("{WASM_OPT_TAG}-{hash}.wasm"));
    let cb = cache.join(format!("{WASM_OPT_TAG}-{hash}.wasm.br"));
    let br = brotli_dst(path);
    if c.is_file() {
        fs::copy(&c, path)?;
        if cb.is_file() {
            fs::copy(&cb, &br)?;
        } else {
            compress_brotli(path, &br)?;
            fs::copy(&br, &cb)?;
        }
        require_file(path, "wasm-opt")?;
        require_file(&br, "brotli")?;
        return Ok(());
    }
    println!("==> wasm-opt -Oz ({})...", path.display());
    let parent = path.parent().unwrap();
    let tmp = tempfile::NamedTempFile::new_in(parent)?;
    run(
        "wasm-opt",
        &[
            "-Oz",
            "--enable-bulk-memory",
            "--enable-nontrapping-float-to-int",
            "--vacuum",
            path.to_str().context("WASM path is not UTF-8")?,
            "-o",
            tmp.path().to_str().context("temporary path is not UTF-8")?,
        ],
        None,
    )?;
    tmp.persist(path)
        .map_err(|e| anyhow::anyhow!("persist: {}", e.error))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o644))?;
    require_file(path, "wasm-opt")?;
    fs::copy(path, &c)?;
    compress_brotli(path, &br)?;
    fs::copy(&br, &cb)?;
    println!("✅ wasm-opt finished");
    Ok(())
}

fn brotli_file(path: &Path) -> Result<()> {
    require_file(path, "brotli source")?;
    let dst = brotli_dst(path);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    if dst.is_file() && fs::metadata(&dst)?.len() > 0 {
        println!("==> Brotli cache hit for {name}");
        return Ok(());
    }
    compress_brotli(path, &dst)?;
    println!(
        "✅ Brotli {name} → {} ({}b)",
        dst.display(),
        fs::metadata(&dst)?.len()
    );
    Ok(())
}

fn minify_js(path: &Path) -> Result<()> {
    require_file(path, "minify input")?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    println!("==> Minifying {name}...");
    let src = fs::read_to_string(path)?;
    if src.trim().is_empty() {
        bail!("minify input empty");
    }
    let m = minifier::js::minify(&src).to_string();
    if m.is_empty() {
        bail!("minify empty");
    }
    fs::write(path, m)?;
    println!("✅ Minifying {name} finished");
    Ok(())
}

fn build_index(
    paths: &Paths,
    out: &Path,
    version: &str,
    js: &str,
    wasm: &str,
    ts: &str,
    cg: bool,
) -> Result<()> {
    let tpl = fs::read_to_string(paths.shell.join("index.html.template"))?;
    let html = tpl
        .replace("__VERSION__", version)
        .replace(
            "./__JS_FILE__",
            &if cg {
                format!("./{js}")
            } else {
                "../__JS_FILE__".to_string()
            },
        )
        .replace(
            "./__WASM_FILE__",
            &if cg {
                format!("./{wasm}")
            } else {
                "../__WASM_FILE__".to_string()
            },
        )
        .replace("__JS_FILE__", js)
        .replace("__WASM_FILE__", wasm)
        .replace("__BUILD_TS__", ts)
        .replace("__ASSETS_UI_BASE__", "/assets/cdn/ui/")
        .replace(
            "href=\"./sow.svg\"",
            if cg {
                "href=\"sow.svg\""
            } else {
                "href=\"../sow.svg\""
            },
        )
        .replace(
            "href=\"./favicon.ico\"",
            if cg {
                "href=\"favicon.ico\""
            } else {
                "href=\"../favicon.ico\""
            },
        )
        .replace(
            "src=\"./loader.js\"",
            if cg {
                "src=\"loader.js\""
            } else {
                "src=\"../loader.js\""
            },
        )
        .replace(
            "src=\"./sdk/store_portals.js\"",
            if cg {
                "src=\"sdk/store_portals.js\""
            } else {
                "src=\"../sdk/store_portals.js\""
            },
        )
        .replace(
            "register('./sw.js', { scope: './' })",
            if cg {
                "register('sw.js', { scope: '/' })"
            } else {
                "register('../sw.js', { scope: '../' })"
            },
        );
    let index = if cg {
        out.join("index.html")
    } else {
        out.join("play/index.html")
    };
    fs::create_dir_all(index.parent().unwrap())?;
    fs::write(&index, &html)?;
    let loader =
        fs::read_to_string(paths.shell.join("loader.js"))?.replace("</script>", "<\\/script>");
    let mut fh = fs::read_to_string(&index)?;
    let marker = "/* __INLINE_LOADER_JS__ */";
    if fh.contains(marker) {
        fh = fh.replacen(marker, &loader, 1);
    } else if fh.contains(r#"<script src="../loader.js"></script>"#) {
        fh = fh.replacen(
            r#"<script src="../loader.js"></script>"#,
            &format!("<script>\n{loader}\n</script>"),
            1,
        );
    } else if fh.contains(r#"<script src="./loader.js"></script>"#) {
        fh = fh.replacen(
            r#"<script src="./loader.js"></script>"#,
            &format!("<script>\n{loader}\n</script>"),
            1,
        );
    } else {
        bail!("index.html: no loader injection point");
    }
    fs::write(&index, fh)?;
    Ok(())
}

fn copy_shell(paths: &Paths, out: &Path) -> Result<()> {
    let fav = paths.shell.join("favicon_io");
    if fav.is_dir() {
        for e in fs::read_dir(&fav)? {
            let e = e?;
            let d = out.join(e.file_name());
            if e.path().is_dir() {
                copy_dir(&e.path(), &d)?;
            } else {
                fs::copy(e.path(), d)?;
            }
        }
    }
    fs::copy(paths.shell.join("sow.svg"), out.join("sow.svg"))?;
    fs::copy(paths.shell.join("loader.js"), out.join("loader.js"))?;
    copy_dir(&paths.shell.join("sdk"), &out.join("sdk"))?;
    Ok(())
}

fn write_sw(out: &Path, version: &str, js: &str, wasm: &str, ts: &str) -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let tpl = fs::read_to_string(root.join("sow-web/shell/sw.js.template"))?;
    fs::write(
        out.join("sw.js"),
        tpl.replace("__VERSION__", version)
            .replace("__JS_FILE__", js)
            .replace("__WASM_FILE__", wasm)
            .replace("__BUILD_TS__", ts),
    )?;
    Ok(())
}

fn write_manifest(out: &Path, version: &str, js: &str, wasm: &str, ts: &str) -> Result<()> {
    fs::write(
        out.join("game-manifest.json"),
        format!(r#"{{"js":"{js}","wasm":"{wasm}","build_ts":"{ts}","version":"{version}"}}"#),
    )?;
    Ok(())
}

fn export_locales(out: &Path) -> Result<()> {
    let d = out.join("locales");
    fs::create_dir_all(&d)?;
    for (l, c) in [
        (&sow_i18n::Language::English, "en"),
        (&sow_i18n::Language::Spanish, "es"),
        (&sow_i18n::Language::French, "fr"),
        (&sow_i18n::Language::German, "de"),
    ] {
        fs::write(d.join(c), serde_json::to_string_pretty(sow_i18n::get(*l))?)?;
    }
    Ok(())
}

fn verify_layout(dir: &Path) -> Result<()> {
    let (mut wn, mut jn) = (None, None);
    for e in fs::read_dir(dir)? {
        let e = e?;
        let n = e.file_name().to_string_lossy().into_owned();
        if n.ends_with("_bg.wasm") && !n.ends_with(".br") {
            wn = Some(n.clone());
        }
        if n.starts_with("sow_client_") && n.ends_with(".js") && !n.ends_with(".br") {
            jn = Some(n);
        }
    }
    let w = wn.as_ref().context("missing _bg.wasm")?;
    let j = jn.as_ref().context("missing sow_client_*.js")?;
    if !dir.join(format!("{w}.br")).is_file() {
        bail!("missing {w}.br");
    }
    if !dir.join(format!("{j}.br")).is_file() {
        bail!("missing {j}.br");
    }
    // Webroot contract: marketing site at the root, game under play/.
    for required in [
        "index.html",
        "play/index.html",
        "robots.txt",
        "sitemap.xml",
        "app.js",
        "styles.css",
        "data.js",
        "sow.svg",
    ] {
        if !dir.join(required).is_file() {
            bail!("webroot missing {}", required);
        }
    }
    if dir.join("admin").exists() {
        bail!("webroot must not contain admin/ (dashboard was removed)");
    }
    println!("✅ Dist layout OK ({})", dir.display());
    Ok(())
}

fn verify_cg_layout(dir: &Path) -> Result<()> {
    for required in [
        "index.html",
        "sow_client.js.br",
        "sow_client_bg.wasm.br",
        "sow.svg",
        "loader.js",
        "sw.js",
        "game-manifest.json",
        "sdk/store_portals.js",
        "locales/en",
    ] {
        if !dir.join(required).is_file() {
            bail!("crazygames bundle missing {}", required);
        }
    }
    // The bundle is a whitelist; these must never ride along again.
    for forbidden in ["maps", "assets", "admin", "play"] {
        if dir.join(forbidden).exists() {
            bail!("crazygames bundle must not contain {forbidden}/");
        }
    }
    let html = fs::read_to_string(dir.join("index.html"))?;
    for needle in [
        "sdk.crazygames.com/crazygames-sdk-v3.js",
        "SOW_MAPS_URL = \"https://shadowsofwar.io/maps\"",
        "SOW_ASSETS_URL = \"https://shadowsofwar.io/assets\"",
        "sow_client.js.br",
        "sow_client_bg.wasm.br",
    ] {
        if !html.contains(needle) {
            bail!("crazygames index.html missing: {}", needle);
        }
    }
    // No uncompressed client artifacts may ship to the portal.
    for e in fs::read_dir(dir)? {
        let n = e?.file_name().to_string_lossy().into_owned();
        if (n.ends_with("_bg.wasm") || n == "sow_client.js") && !n.ends_with(".br") {
            bail!("crazygames bundle contains uncompressed artifact {n}");
        }
    }
    Ok(())
}

fn package_self(paths: &Paths, out: &Path, version: &str) -> Result<()> {
    if out.exists() {
        for e in fs::read_dir(out)? {
            let e = e?;
            let p = e.path();
            if p.is_dir() {
                fs::remove_dir_all(&p)?;
            } else {
                fs::remove_file(&p)?;
            }
        }
    } else {
        fs::create_dir_all(out)?;
    }

    let hash = file_sha256(&paths.wasm_input)?;
    let ts = hash[..10].to_string();
    let js = format!("sow_client_{ts}.js");
    let wasm = format!("sow_client_{ts}_bg.wasm");

    let cdn = out.join("assets/cdn");
    fs::create_dir_all(&cdn)?;
    if paths.assets_cdn.is_dir() {
        copy_dir(&paths.assets_cdn, &cdn)?;
    }
    let maps = out.join("maps");
    fs::create_dir_all(&maps)?;
    if paths.assets_maps.is_dir() {
        copy_dir(&paths.assets_maps, &maps)?;
    }

    run_bindgen(&paths.wasm_input, out, &format!("sow_client_{ts}"))?;
    copy_shell(paths, out)?;
    build_index(paths, out, version, &js, &wasm, &ts, false)?;
    export_locales(out)?;

    minify_js(&out.join(&js))?;
    run_wasm_opt(&out.join(&wasm), &paths.wasm_cache)?;
    brotli_file(&out.join(&wasm))?;
    brotli_file(&out.join(&js))?;
    write_sw(out, version, &js, &wasm, &ts)?;
    write_manifest(out, version, &js, &wasm, &ts)?;

    // Marketing website at the webroot root (game shell lives under play/).
    let site = paths.root.join("sow-web/site");
    for name in ["index.html", "app.js", "styles.css", "data.js"] {
        let src = site.join(name);
        if !src.is_file() {
            bail!("website source missing: {}", src.display());
        }
        fs::copy(&src, out.join(name))?;
    }
    fs::write(
        out.join("robots.txt"),
        "User-agent: *\nAllow: /\n\nSitemap: https://shadowsofwar.io/sitemap.xml\n",
    )?;
    fs::write(
        out.join("sitemap.xml"),
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
            "  <url><loc>https://shadowsofwar.io/</loc><changefreq>weekly</changefreq><priority>1.0</priority></url>\n",
            "  <url><loc>https://shadowsofwar.io/play/</loc><changefreq>weekly</changefreq><priority>0.9</priority></url>\n",
            "</urlset>\n",
        ),
    )?;
    println!("✅ Website staged at webroot root");

    prune_qs(out)?;
    verify_layout(out)?;
    println!("Load paths: {wasm}, {js}");
    Ok(())
}

fn package_cg(play_dir: &Path, out: &Path, paths: &Paths, version: &str) -> Result<()> {
    // The portal bundle is a strict whitelist: index.html, the brotli client
    // pair, and the shell essentials. Everything else (maps, assets, admin)
    // streams from the production CDN at runtime. Never clone dist/web here.
    let (mut jh, mut wh) = (String::new(), String::new());
    for e in fs::read_dir(play_dir)? {
        let n = e?.file_name().to_string_lossy().into_owned();
        if n.starts_with("sow_client_") && n.ends_with(".js") && !n.ends_with(".br") {
            jh = n
                .trim_start_matches("sow_client_")
                .trim_end_matches(".js")
                .to_string();
        }
        if n.ends_with("_bg.wasm") && !n.ends_with(".br") {
            wh = n
                .trim_end_matches("_bg.wasm")
                .trim_start_matches("sow_client_")
                .to_string();
        }
    }
    if jh.is_empty() || wh.is_empty() {
        bail!("CrazyGames package is missing hashed client artifacts");
    }

    if out.exists() {
        fs::remove_dir_all(out)?;
    }
    fs::create_dir_all(out)?;

    fs::copy(
        play_dir.join(format!("sow_client_{jh}.js.br")),
        out.join("sow_client.js.br"),
    )?;
    fs::copy(
        play_dir.join(format!("sow_client_{wh}_bg.wasm.br")),
        out.join("sow_client_bg.wasm.br"),
    )?;
    copy_shell(paths, out)?;
    export_locales(out)?;
    write_sw(
        out,
        version,
        "sow_client.js.br",
        "sow_client_bg.wasm.br",
        &jh,
    )?;
    fs::write(
        out.join("game-manifest.json"),
        format!(
            r#"{{"js":"sow_client.js.br","wasm":"sow_client_bg.wasm.br","build_ts":"{jh}","version":"{version}"}}"#
        ),
    )?;
    build_index(
        paths,
        out,
        version,
        &format!("sow_client_{jh}.js"),
        &format!("sow_client_{wh}_bg.wasm"),
        &jh,
        true,
    )?;

    // Patch index.html: hashed names -> stable .br names, inject portal SDK
    // and boot overrides (maps AND assets resolve against the prod CDN).
    let idx = out.join("index.html");
    let html = fs::read_to_string(&idx)?;
    let mut lines: Vec<String> = html.lines().map(String::from).collect();
    let (mut sdk, mut boot) = (false, false);
    for line in &mut lines {
        if line.contains("PORTAL_SDK_SLOT") {
            *line = "    <script src=\"https://sdk.crazygames.com/crazygames-sdk-v3.js\"></script>"
                .to_string();
            sdk = true;
        } else if line.contains("PORTAL_BOOT_SLOT") {
            *line = "        window.SOW_PORTAL = \"crazygames\"; window.SOW_WS_URL = \"wss://shadowsofwar.io/ws/\"; window.SOW_MAPS_URL = \"https://shadowsofwar.io/maps\"; window.SOW_ASSETS_URL = \"https://shadowsofwar.io/assets\";".to_string();
            boot = true;
        }
    }
    if !sdk || !boot {
        bail!("CrazyGames index.html is missing portal slots (sdk={sdk} boot={boot})");
    }
    let html_out = lines
        .join("\n")
        .replace(&format!("sow_client_{jh}.js"), "sow_client.js.br")
        .replace(&format!("sow_client_{wh}_bg.wasm"), "sow_client_bg.wasm.br");
    fs::write(&idx, html_out)?;

    verify_cg_layout(out)?;
    println!("✅ CrazyGames bundle ready (whitelist): {}", out.display());
    Ok(())
}

fn cmd_native(paths: &Paths) -> Result<()> {
    let mut c = Command::new("cargo");
    c.args(["run", "--bin", "client", "--"])
        .current_dir(&paths.root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .env("VERBOSE", "1");
    if !c.spawn()?.wait()?.success() {
        bail!("client failed");
    }
    Ok(())
}

fn load_dotenv(path: &Path) {
    if path.is_file()
        && let Ok(c) = fs::read_to_string(path)
    {
        for line in c.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(eq) = line.find('=') {
                unsafe {
                    env::set_var(line[..eq].trim(), line[eq + 1..].trim().trim_matches('"'));
                }
            }
        }
    }
}

/// Create the machine-to-machine relay control secret once, locally, when
/// the ignored deployment environment does not have one yet.  The value is
/// persisted only in sow-dist/.env (mode 0600) and is staged to both ends by
/// ./sow p; it is never included in a command line or pipeline output.
fn ensure_generated_secret(root: &Path, key: &str) -> Result<()> {
    if env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_some()
    {
        return Ok(());
    }

    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let value = hex::encode(bytes);
    let path = root.join("sow-dist/.env");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open {} for generated secret", path.display()))?;
    file.write_all(format!("\n{key}={value}\n").as_bytes())?;
    file.sync_all()?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    unsafe { env::set_var(key, value) };
    println!("✅ generated and persisted {key} in ignored sow-dist/.env");
    Ok(())
}

/// Rotate both deployment credentials in the ignored local environment.
/// Values never enter command-line arguments or stdout; the caller must run
/// the normal production pipeline immediately afterwards.
fn rotate_deployment_secrets(root: &Path) -> Result<()> {
    let path = root.join("sow-dist/.env");
    let input = fs::read_to_string(&path)
        .with_context(|| format!("read {} for secret rotation", path.display()))?;
    let mut output = String::with_capacity(input.len() + 160);
    let mut replaced = [false; 2];
    for line in input.lines() {
        let key = line.split_once('=').map(|(key, _)| key.trim());
        let slot = match key {
            Some("SOW_DB_SECRET") => Some(0),
            Some("SOW_RELAY_CONTROL_SECRET") => Some(1),
            _ => None,
        };
        if let Some(slot) = slot {
            let mut bytes = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            let name = if slot == 0 {
                "SOW_DB_SECRET"
            } else {
                "SOW_RELAY_CONTROL_SECRET"
            };
            output.push_str(name);
            output.push('=');
            output.push_str(&hex::encode(bytes));
            output.push('\n');
            replaced[slot] = true;
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    for (slot, present) in replaced.iter().enumerate() {
        if !present {
            let mut bytes = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            let name = if slot == 0 {
                "SOW_DB_SECRET"
            } else {
                "SOW_RELAY_CONTROL_SECRET"
            };
            output.push_str(name);
            output.push('=');
            output.push_str(&hex::encode(bytes));
            output.push('\n');
        }
    }
    let parent = path
        .parent()
        .context("secret environment has no parent directory")?;
    let mut temp =
        tempfile::NamedTempFile::new_in(parent).context("create temporary secret environment")?;
    temp.write_all(output.as_bytes())?;
    temp.as_file().sync_all()?;
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o600))?;
    temp.persist(&path)
        .map_err(|e| anyhow::anyhow!("persist rotated secret environment: {}", e.error))?;
    println!("✅ rotated deployment secrets locally (values redacted)");
    Ok(())
}

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()?;
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--rotate-secrets") {
        rotate_deployment_secrets(&root)?;
    }
    load_dotenv(&root.join("sow-dist/.env"));
    ensure_generated_secret(&root, "SOW_RELAY_CONTROL_SECRET")?;

    let paths = Paths::discover()?;
    let mut cmd = String::new();
    let mut bump = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-v" | "--version" => bump = true,
            "--rotate-secrets" => {}
            _ if cmd.is_empty() => cmd = args[i].clone(),
            other => bail!("unknown argument: {other}"),
        }
        i += 1;
    }

    match cmd.as_str() {
        "p" | "prod" | "play" => prod::execute(&paths, bump),
        "native" | "n" | "" => cmd_native(&paths),
        _ => {
            eprintln!("Usage: ./sow [p|native]");
            std::process::exit(1);
        }
    }
}
