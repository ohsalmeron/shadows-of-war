use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{env, fs};

const IONOS_HOST: &str = "root@74.208.246.177";
const IONOS_JAIL: &str = "/zroot/jails/sow-web/var/www/shadowsofwar.io/";
const WASM_OPT_TAG: &str = "oz-v1";

fn run(cmd: &str, args: &[&str], cwd: Option<&Path>) -> Result<()> {
    println!("+ {cmd} {}", args.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" "));
    let mut c = Command::new(cmd);
    c.args(args).stdout(Stdio::inherit()).stderr(Stdio::inherit());
    if let Some(d) = cwd { c.current_dir(d); }
    if !c.spawn()?.wait()?.success() { bail!("{cmd} failed"); }
    Ok(())
}

fn output(cmd: &str, args: &[&str]) -> Result<String> {
    let o = Command::new(cmd).args(args).output()?;
    if !o.status.success() { bail!("{cmd} failed: {}", String::from_utf8_lossy(&o.stderr)); }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn shell_quote(s: &str) -> String {
    if s.is_empty() { return "''".to_string(); }
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "./_=-+:,".contains(c)) { s.to_string() }
    else { format!("'{}'", s.replace('\'', "'\\''")) }
}

struct Paths {
    root: PathBuf, shell: PathBuf, target_dir: PathBuf,
    assets_cdn: PathBuf, assets_maps: PathBuf, assets_static: PathBuf,
    dist_play: PathBuf, dist_cg: PathBuf, dist_dev: PathBuf,
    wasm_input: PathBuf, wasm_cache: PathBuf, state_dir: PathBuf,
}

impl Paths {
    fn discover() -> Result<Self> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").canonicalize()?;
        let t = env::var("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|_| root.join("target"));
        let s = root.join("dist/.sow-state");
        Ok(Self {
            wasm_input: t.join("wasm32-unknown-unknown/wasm-release/sow_client.wasm"),
            wasm_cache: s.join("wasm-opt-cache"),
            shell: root.join("sow-web/shell"),
            assets_cdn: root.join("assets/cdn"), assets_maps: root.join("assets/maps"),
            assets_static: root.join("assets/static"),
            dist_play: root.join("dist/play"), dist_cg: root.join("dist/crazygames"),
            dist_dev: root.join("dist/site-dev"), state_dir: s, target_dir: t, root,
        })
    }
}

fn file_sha256(path: &Path) -> Result<String> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new(); let mut b = [0u8; 65536];
    loop { let n = f.read(&mut b)?; if n == 0 { break; } h.update(&b[..n]); }
    Ok(format!("{:x}", h.finalize()))
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for e in fs::read_dir(src)? {
        let e = e?; let to = dst.join(e.file_name());
        if e.path().is_dir() { copy_dir(&e.path(), &to)?; } else { fs::copy(e.path(), to)?; }
    }
    Ok(())
}

fn copy_dir_ex(src: &Path, dst: &Path, ex: &[&str]) -> Result<()> {
    fs::create_dir_all(dst)?;
    for e in fs::read_dir(src)? {
        let e = e?; let n = e.file_name().to_string_lossy().to_string();
        if ex.contains(&n.as_str()) { continue; }
        let to = dst.join(e.file_name());
        if e.path().is_dir() { copy_dir(&e.path(), &to)?; } else { fs::copy(e.path(), to)?; }
    }
    Ok(())
}

fn hash_tree(h: &mut Sha256, dir: &Path, exts: &[&str]) -> Result<()> {
    if !dir.is_dir() { return Ok(()); }
    let mut e: Vec<_> = fs::read_dir(dir)?.filter_map(Result::ok).map(|e| e.path()).collect();
    e.sort();
    for p in e {
        if p.is_dir() { hash_tree(h, &p, exts)?; }
        else if p.extension().and_then(|e| e.to_str()).is_some_and(|e| exts.contains(&e)) {
            h.update(fs::read(&p)?);
        }
    }
    Ok(())
}

fn require_file(path: &Path, label: &str) -> Result<()> {
    if !path.is_file() { bail!("{label} missing: {}", path.display()); }
    if fs::metadata(path)?.len() == 0 { bail!("{label} empty: {}", path.display()); }
    Ok(())
}

fn brotli_dst(path: &Path) -> PathBuf { PathBuf::from(format!("{}.br", path.display())) }

fn compress_brotli(src: &Path, dst: &Path) -> Result<()> {
    let input = fs::read(src)?;
    if input.is_empty() { bail!("brotli source empty"); }
    let mut out = Vec::new();
    let mut w = brotli::CompressorWriter::new(&mut out, 4096, 9, 22);
    w.write_all(&input)?; w.flush()?; drop(w);
    fs::write(dst, &out)?;
    Ok(())
}

fn prune_qs(root: &Path) -> Result<()> {
    let mut n = 0u32;
    for e in walkdir::WalkDir::new(root) {
        let e = e?;
        if e.file_type().is_file() && e.file_name().to_string_lossy().contains("?v=") {
            fs::remove_file(e.path())?; n += 1;
        }
    }
    if n > 0 { println!("Removed {n} querystring artifact(s)"); }
    Ok(())
}

fn compile_wasm(paths: &Paths, dev: bool) -> Result<()> {
    println!("==> Compiling WASM (wasm-release)...");
    let mut a = vec!["build", "--profile", "wasm-release", "-p", "sow-client", "--target", "wasm32-unknown-unknown"];
    if dev { a.extend_from_slice(&["--features", "dev"]); println!("==> (local) dev tools enabled"); }
    let mut c = Command::new("cargo");
    c.args(&a).current_dir(&paths.root).stdout(Stdio::inherit()).stderr(Stdio::inherit());
    c.env("RUSTFLAGS", "-C target-feature=-bulk-memory");
    if !c.spawn()?.wait()?.success() { bail!("WASM compile failed"); }
    require_file(&paths.wasm_input, "WASM output")?;
    Ok(())
}

fn run_bindgen(wasm: &Path, out: &Path, name: &str) -> Result<()> {
    println!("==> Running wasm-bindgen for {name}...");
    wasm_bindgen_cli_support::Bindgen::new()
        .input_path(wasm).web(true)?.out_name(name).typescript(false)
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
        if cb.is_file() { fs::copy(&cb, &br)?; } else { compress_brotli(path, &br)?; fs::copy(&br, &cb)?; }
        require_file(path, "wasm-opt")?; require_file(&br, "brotli")?; return Ok(());
    }
    println!("==> wasm-opt -Oz ({})...", path.display());
    let parent = path.parent().unwrap();
    let tmp = tempfile::NamedTempFile::new_in(parent)?;
    let mut o = wasm_opt::OptimizationOptions::new_optimize_for_size_aggressively();
    o.enable_feature(wasm_opt::Feature::BulkMemory);
    o.enable_feature(wasm_opt::Feature::TruncSat);
    o.add_pass(wasm_opt::Pass::Vacuum);
    o.run(path, tmp.path()).map_err(|e| anyhow::anyhow!("wasm-opt: {e}"))?;
    tmp.persist(path).map_err(|e| anyhow::anyhow!("persist: {}", e.error))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o644))?;
    require_file(path, "wasm-opt")?;
    fs::copy(path, &c)?; compress_brotli(path, &br)?; fs::copy(&br, &cb)?;
    println!("✅ wasm-opt finished");
    Ok(())
}

fn brotli_file(path: &Path) -> Result<()> {
    require_file(path, "brotli source")?;
    let dst = brotli_dst(path);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    if dst.is_file() && fs::metadata(&dst)?.len() > 0 {
        println!("==> Brotli cache hit for {name}"); return Ok(());
    }
    compress_brotli(path, &dst)?;
    println!("✅ Brotli {name} → {} ({}b)", dst.display(), fs::metadata(&dst)?.len());
    Ok(())
}

fn minify_js(path: &Path) -> Result<()> {
    require_file(path, "minify input")?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    println!("==> Minifying {name}...");
    let src = fs::read_to_string(path)?;
    if src.trim().is_empty() { bail!("minify input empty"); }
    let m = minifier::js::minify(&src).to_string();
    if m.is_empty() { bail!("minify empty"); }
    fs::write(path, m)?; println!("✅ Minifying {name} finished"); Ok(())
}

fn build_index(paths: &Paths, out: &Path, version: &str, js: &str, wasm: &str, ts: &str, cg: bool) -> Result<()> {
    let tpl = fs::read_to_string(paths.shell.join("index.html.template"))?;
    let html = tpl
        .replace("__VERSION__", version)
        .replace("./__JS_FILE__", &if cg { format!("./{js}") } else { "../__JS_FILE__".to_string() })
        .replace("./__WASM_FILE__", &if cg { format!("./{wasm}") } else { "../__WASM_FILE__".to_string() })
        .replace("__JS_FILE__", js)
        .replace("__WASM_FILE__", wasm)
        .replace("__BUILD_TS__", ts)
        .replace("__ASSETS_UI_BASE__", "/assets/cdn/ui/")
        .replace("href=\"./sow.svg\"", &if cg { "href=\"sow.svg\"" } else { "href=\"../sow.svg\"" })
        .replace("href=\"./favicon.ico\"", &if cg { "href=\"favicon.ico\"" } else { "href=\"../favicon.ico\"" })
        .replace("src=\"./loader.js\"", &if cg { "src=\"loader.js\"" } else { "src=\"../loader.js\"" })
        .replace("src=\"./sdk/store_portals.js\"", &if cg { "src=\"sdk/store_portals.js\"" } else { "src=\"../sdk/store_portals.js\"" })
        .replace("register('./sw.js', { scope: './' })", &if cg { "register('sw.js', { scope: '/' })" } else { "register('../sw.js', { scope: '../' })" });
    let index = out.join("play/index.html");
    fs::create_dir_all(index.parent().unwrap())?;
    fs::write(&index, &html)?;
    let loader = fs::read_to_string(paths.shell.join("loader.js"))?.replace("</script>", "<\\/script>");
    let mut fh = fs::read_to_string(&index)?;
    let marker = "/* __INLINE_LOADER_JS__ */";
    if fh.contains(marker) { fh = fh.replacen(marker, &loader, 1); }
    else if fh.contains(r#"<script src="../loader.js"></script>"#) { fh = fh.replacen(r#"<script src="../loader.js"></script>"#, &format!("<script>\n{loader}\n</script>"), 1); }
    else if fh.contains(r#"<script src="./loader.js"></script>"#) { fh = fh.replacen(r#"<script src="./loader.js"></script>"#, &format!("<script>\n{loader}\n</script>"), 1); }
    else { bail!("index.html: no loader injection point"); }
    fs::write(&index, fh)?;
    Ok(())
}

fn copy_shell(paths: &Paths, out: &Path) -> Result<()> {
    let fav = paths.shell.join("favicon_io");
    if fav.is_dir() {
        for e in fs::read_dir(&fav)? {
            let e = e?; let d = out.join(e.file_name());
            if e.path().is_dir() { copy_dir(&e.path(), &d)?; } else { fs::copy(e.path(), d)?; }
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
    fs::write(out.join("sw.js"), tpl
        .replace("__VERSION__", version)
        .replace("__JS_FILE__", js)
        .replace("__WASM_FILE__", wasm)
        .replace("__BUILD_TS__", ts))?;
    Ok(())
}

fn write_manifest(out: &Path, version: &str, js: &str, wasm: &str, ts: &str) -> Result<()> {
    fs::write(out.join("game-manifest.json"), format!(r#"{{"js":"{js}","wasm":"{wasm}","build_ts":"{ts}","version":"{version}"}}"#))?;
    Ok(())
}

fn export_locales(out: &Path) -> Result<()> {
    let d = out.join("locales"); fs::create_dir_all(&d)?;
    for (l, c) in [(&sow_i18n::Language::English, "en"), (&sow_i18n::Language::Spanish, "es"), (&sow_i18n::Language::French, "fr"), (&sow_i18n::Language::German, "de")] {
        fs::write(d.join(c), serde_json::to_string_pretty(sow_i18n::get(*l))?)?;
    }
    Ok(())
}

fn verify_layout(dir: &Path) -> Result<()> {
    let (mut wn, mut jn) = (None, None);
    for e in fs::read_dir(dir)? {
        let e = e?; let n = e.file_name().to_string_lossy().into_owned();
        if n.ends_with("_bg.wasm") && !n.ends_with(".br") { wn = Some(n.clone()); }
        if n.starts_with("sow_client_") && n.ends_with(".js") && !n.ends_with(".br") { jn = Some(n); }
    }
    let w = wn.as_ref().context("missing _bg.wasm")?;
    let j = jn.as_ref().context("missing sow_client_*.js")?;
    if !dir.join(format!("{w}.br")).is_file() { bail!("missing {w}.br"); }
    if !dir.join(format!("{j}.br")).is_file() { bail!("missing {j}.br"); }
    println!("✅ Dist layout OK ({})", dir.display());
    Ok(())
}

fn is_up_to_date(paths: &Paths) -> Result<bool> {
    let cache = paths.state_dir.join("build-hash");
    let mut h = Sha256::new();
    hash_tree(&mut h, &paths.shell, &["html", "template", "js", "svg", "rs"])?;
    let hash = format!("{:x}", h.finalize());
    if let Ok(c) = fs::read_to_string(&cache) {
        if c.trim() == hash && paths.dist_play.join("game-manifest.json").is_file() { return Ok(true); }
    }
    fs::create_dir_all(&paths.state_dir)?;
    fs::write(&cache, &hash)?;
    Ok(false)
}

fn package_self(paths: &Paths, out: &Path, version: &str) -> Result<()> {
    if out.exists() {
        for e in fs::read_dir(out)? {
            let e = e?; let p = e.path();
            if p.is_dir() { fs::remove_dir_all(&p)?; } else { fs::remove_file(&p)?; }
        }
    } else { fs::create_dir_all(out)?; }

    let hash = file_sha256(&paths.wasm_input) ?;
    let ts = hash[..10].to_string();
    let js = format!("sow_client_{ts}.js");
    let wasm = format!("sow_client_{ts}_bg.wasm");

    let cdn = out.join("assets/cdn"); fs::create_dir_all(&cdn)?;
    if paths.assets_cdn.is_dir() { copy_dir(&paths.assets_cdn, &cdn)?; }
    let maps = out.join("maps"); fs::create_dir_all(&maps)?;
    if paths.assets_maps.is_dir() { copy_dir(&paths.assets_maps, &maps)?; }

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

    let admin = out.join("admin/dashboard");
    fs::create_dir_all(&admin)?;
    let src = paths.root.join("sow-server/src/admin_dashboard.html");
    if src.is_file() { fs::copy(&src, admin.join("index.html"))?; println!("✅ Admin dashboard copied"); }
    else { eprintln!("⚠️  Admin dashboard not found"); }

    prune_qs(out)?;
    verify_layout(out)?;
    println!("Load paths: {wasm}, {js}");
    Ok(())
}

fn package_cg(play_dir: &Path, out: &Path, paths: &Paths) -> Result<()> {
    if out.exists() { fs::remove_dir_all(out)?; }
    copy_dir(play_dir, out)?;

    // Find hash names in the cloned output, rename to un-hashed
    let (mut jh, mut wh) = (String::new(), String::new());
    let mut entries: Vec<_> = fs::read_dir(out)?.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in &entries {
        let n = e.file_name().to_string_lossy().to_string();
        if n.starts_with("sow_client_") && n.ends_with(".js") && !n.ends_with(".br") && jh.is_empty() {
            jh = n.trim_start_matches("sow_client_").trim_end_matches(".js").to_string();
            fs::rename(e.path(), out.join("sow_client.js"))?;
        }
        if n.ends_with("_bg.wasm") && !n.ends_with(".br") && wh.is_empty() {
            wh = n.trim_end_matches("_bg.wasm").trim_start_matches("sow_client_").to_string();
            fs::rename(e.path(), out.join("sow_client_bg.wasm"))?;
        }
    }
    // Rename .br sidecars
    if !jh.is_empty() {
        let br = out.join(format!("sow_client_{jh}.js.br"));
        if br.is_file() { fs::rename(&br, out.join("sow_client.js.br"))?; }
    }
    if !wh.is_empty() {
        let br = out.join(format!("sow_client_{wh}_bg.wasm.br"));
        if br.is_file() { fs::rename(&br, out.join("sow_client_bg.wasm.br"))?; }
    }

    // Patch index.html: inject SDK, replace PORTAL slots
    let idx = out.join("play/index.html");
    let html = fs::read_to_string(&idx)?;
    let mut lines: Vec<String> = html.lines().map(String::from).collect();
    for line in &mut lines {
        if line.contains("PORTAL_SDK_SLOT") {
            *line = "    <script src=\"https://sdk.crazygames.com/crazygames-sdk-v3.js\"></script>".to_string();
        } else if line.contains("PORTAL_BOOT_SLOT") {
            *line = "        window.SOW_PORTAL = \"crazygames\"; window.SOW_WS_URL = \"wss://shadowsofwar.io/ws/\"; window.SOW_MAPS_URL = \"https://shadowsofwar.io/maps\";".to_string();
        }
    }
    let mut html_out = lines.join("\n");
    // Fix JS/WASM references to .br
    html_out = html_out.replace("sow_client.js", "sow_client.js.br");
    html_out = html_out.replace("sow_client_bg.wasm", "sow_client_bg.wasm.br");
    fs::write(&idx, html_out)?;

    // Remove uncompressed wasm/js (keep only .br)
    for e in fs::read_dir(out)? {
        let e = e?; let n = e.file_name().to_string_lossy().to_string();
        if (n.ends_with("_bg.wasm") || (n.starts_with("sow_client_") && n.ends_with(".js"))) && !n.ends_with(".br") {
            let _ = fs::remove_file(e.path());
        }
    }

    // Stage static assets (no maps)
    let sd = out.join("assets/static");
    if sd.exists() { fs::remove_dir_all(&sd)?; }
    fs::create_dir_all(sd.parent().unwrap())?;
    copy_dir_ex(&paths.assets_static, &sd, &["maps"])?;
    println!("✅ CrazyGames bundle ready: {}", out.display());
    Ok(())
}

fn verify_deploy() -> Result<()> {
    let m = output("curl", &["-sSf", "https://shadowsofwar.io/game-manifest.json"])?;
    if !m.contains("sow_client") { bail!("manifest invalid"); }
    let h = output("curl", &["-sSf", "-o", "/dev/null", "-w", "%{http_code}", "https://shadowsofwar.io/play/"])?;
    if h != "200" { bail!("/play/ returned {h}"); }
    let a = output("curl", &["-sSf", "-o", "/dev/null", "-w", "%{http_code}", "https://shadowsofwar.io/admin/dashboard"])?;
    println!("  /play/ HTTP {h}  /admin/dashboard HTTP {a}");
    if a == "200" { println!("✅ Admin dashboard OK"); }
    Ok(())
}

fn cmd_native(paths: &Paths) -> Result<()> {
    let mut c = Command::new("cargo");
    c.args(["run", "--bin", "client", "--"]).current_dir(&paths.root)
        .stdout(Stdio::inherit()).stderr(Stdio::inherit()).env("VERBOSE", "1");
    if !c.spawn()?.wait()?.success() { bail!("client failed"); }
    Ok(())
}

fn cmd_local(paths: &Paths, port: u16, build_only: bool) -> Result<()> {
    let v = fs::read_to_string(paths.root.join(".version")).unwrap_or_default().trim().to_string();
    println!("==> local v{v}");

    compile_wasm(paths, true)?;
    let hash = file_sha256(&paths.wasm_input)?;
    let ts = hash[..10].to_string();

    if paths.dist_dev.exists() { fs::remove_dir_all(&paths.dist_dev)?; }
    fs::create_dir_all(&paths.dist_dev)?;

    run_bindgen(&paths.wasm_input, &paths.dist_dev, "sow_client")?;
    copy_shell(paths, &paths.dist_dev)?;

    // Build index.html with prod WS
    let tpl = fs::read_to_string(paths.shell.join("index.html.template"))?;
    let html = tpl
        .replace("__VERSION__", &v)
        .replace("__JS_FILE__", "sow_client.js")
        .replace("__WASM_FILE__", "sow_client_bg.wasm")
        .replace("__BUILD_TS__", &ts)
        .replace("__ASSETS_UI_BASE__", "/assets/cdn/ui/");
    let idx = paths.dist_dev.join("index.html");
    let mut lines: Vec<String> = html.lines().map(String::from).collect();
    for line in &mut lines {
        if line.contains("PORTAL_SDK_SLOT") { *line = String::new(); }
        else if line.contains("PORTAL_BOOT_SLOT") {
            *line = "        window.SOW_PORTAL = \"site\"; window.SOW_WS_URL = \"wss://shadowsofwar.io/ws/\"; window.SOW_MAPS_URL = \"https://shadowsofwar.io/maps\"; window.SOW_ASSETS_URL = \"/assets\";".to_string();
        }
    }
    let mut fh = lines.join("\n");
    let loader = fs::read_to_string(paths.shell.join("loader.js"))?.replace("</script>", "<\\/script>");
    let marker = "/* __INLINE_LOADER_JS__ */";
    if fh.contains(marker) { fh = fh.replacen(marker, &loader, 1); }
    else if fh.contains(r#"<script src="./loader.js"></script>"#) { fh = fh.replacen(r#"<script src="./loader.js"></script>"#, &format!("<script>{loader}</script>"), 1); }
    fs::write(&idx, fh)?;

    println!("Local ready: {}", paths.dist_dev.display());
    if build_only { println!("  --build-only: skipped server"); return Ok(()); }
    println!("  → http://127.0.0.1:{port}/");
    serve_static(&paths.dist_dev, port)
}

fn cmd_prod(paths: &Paths, bump: bool) -> Result<()> {
    let v = if bump {
        let p = paths.root.join(".version");
        let old = fs::read_to_string(&p).unwrap_or_else(|_| "0.1.0".to_string());
        let parts: Vec<&str> = old.trim().split('.').collect();
        let patch: u32 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        let new = format!("{}.{}.{}", parts.first().unwrap_or(&"0"), parts.get(1).unwrap_or(&"1"), patch + 1);
        fs::write(&p, &new)?;
        println!("==> Version bumped {} → {}", old.trim(), new);
        new
    } else {
        fs::read_to_string(paths.root.join(".version")).unwrap_or_else(|_| "0.0.0".to_string()).trim().to_string()
    };
    println!("==> prod v{v}");

    if is_up_to_date(paths)? {
        println!("==> Build unchanged — skipping compile");
    } else {
        compile_wasm(paths, false)?;
        package_self(paths, &paths.dist_play, &v)?;
    }

    println!("==> Phase: CrazyGames bundle");
    package_cg(&paths.dist_play, &paths.dist_cg, paths)?;

    println!("==> Phase: rsync to IONOS");
    let src = format!("{}/", paths.dist_play.to_str().unwrap().trim_end_matches('/'));
    run("rsync", &["-az", "--delete", &src, &format!("{IONOS_HOST}:{IONOS_JAIL}")], Some(&paths.root))?;
    println!("✅ sow-web jail sync OK");

    println!("==> Phase: verify");
    let _ = verify_deploy();
    println!("✅ Prod deployed v{v}");
    Ok(())
}

fn cmd_backfill(paths: &Paths, clouding: bool, deploy: bool, min_fill: usize, max_fill: usize, url: &str) -> Result<()> {
    if clouding {
        run("cargo", &["build", "--release", "-p", "sow-backfill", "--manifest-path", "sow-backfill/Cargo.toml"], Some(&paths.root))?;
        let bin = paths.target_dir.join("release/sow-backfill");
        if !bin.is_file() { bail!("binary not found"); }
        run("scp", &[bin.to_str().unwrap(), "clouding@93.189.88.176:/opt/shadowsofwar/services/matchmaking/backfill/bin/"], None)?;
        let remote_cmd = format!("/opt/shadowsofwar/services/matchmaking/backfill/bin/sow-backfill --min-fill {min_fill} --max-fill {max_fill} --url {url}");
        run("ssh", &["clouding@93.189.88.176", &remote_cmd], None)?;
        println!("✅ Backfill running on Clouding (min={min_fill}% max={max_fill}%)");
    } else if deploy {
        run("cargo", &["build", "--release", "-p", "sow-backfill", "--manifest-path", "sow-backfill/Cargo.toml"], Some(&paths.root))?;
        let bin = paths.target_dir.join("release/sow-backfill");
        if !bin.is_file() { bail!("binary not found"); }
        run("scp", &[bin.to_str().unwrap(), "clouding@93.189.88.176:/opt/shadowsofwar/services/matchmaking/backfill/bin/"], None)?;
        run("ssh", &["clouding@93.189.88.176", "sudo", "systemctl", "restart", "sow-backfill"], None)?;
        println!("✅ Backfill deployed to Clouding (service)");
    } else {
        run("cargo", &["run", "--release", "--manifest-path", "sow-backfill/Cargo.toml", "--", "--min-fill", &min_fill.to_string(), "--max-fill", &max_fill.to_string(), "--url", url], Some(&paths.root))?;
    }
    Ok(())
}

fn serve_static(dir: &Path, port: u16) -> Result<()> {
    use axum::Router;
    use tower_http::services::ServeDir;
    let rt = tokio::runtime::Builder::new_current_thread().enable_io().build()?;
    rt.block_on(async {
        let app = Router::new().nest_service("/", ServeDir::new(dir));
        let addr = format!("127.0.0.1:{port}");
        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        println!("  → http://{addr}/");
        axum::serve(listener, app).await.unwrap();
        Ok::<_, anyhow::Error>(())
    })
}

fn load_dotenv(path: &Path) {
    if path.is_file() {
        if let Ok(c) = fs::read_to_string(path) {
            for line in c.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                if let Some(eq) = line.find('=') {
                    unsafe { env::set_var(line[..eq].trim(), line[eq + 1..].trim().trim_matches('"')); }
                }
            }
        }
    }
}

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").canonicalize()?;
    load_dotenv(&root.join("sow-dist/.env"));

    let paths = Paths::discover()?;
    let args: Vec<String> = env::args().skip(1).collect();
    let mut cmd = String::new();
    let mut port = 8787u16;
    let mut build_only = false;
    let mut deploy = false;
    let mut bump = false;
    let mut clouding = false;
    let mut min_fill = 25usize;
    let mut max_fill = 45usize;
    let mut url = "wss://ws.shadowsofwar.io/ws/".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-p" | "--port" => { i += 1; if i < args.len() { port = args[i].parse().unwrap_or(8787); } }
            "--build-only" => build_only = true,
            "--deploy" => deploy = true,
            "-v" | "--version" => bump = true,
            "-c" | "--clouding" => clouding = true,
            "--min" => { i += 1; if i < args.len() { min_fill = args[i].parse().unwrap_or(25); } }
            "--max" => { i += 1; if i < args.len() { max_fill = args[i].parse().unwrap_or(45); } }
            "--url" => { i += 1; if i < args.len() { url = args[i].clone(); } }
            _ if cmd.is_empty() => cmd = args[i].clone(),
            _ => {}
        }
        i += 1;
    }

    match cmd.as_str() {
        "l" | "local" | "localsite" | "ls" => cmd_local(&paths, port, build_only),
        "p" | "prod" | "play" => cmd_prod(&paths, bump),
        "backfill" | "bf" => cmd_backfill(&paths, clouding, deploy, min_fill, max_fill, &url),
        "native" | "n" | "" => cmd_native(&paths),
        _ => { eprintln!("Usage: ./sow [l|p|backfill|native]"); std::process::exit(1); }
    }
}
