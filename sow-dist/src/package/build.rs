use super::Profile;
use crate::config::DeployConfig;
use crate::package::verify::verify_layout;
use crate::paths::{Paths, PORTAL_JS, PORTAL_WASM};
use crate::wasm;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

pub fn build(
    paths: &Paths,
    profile: Profile,
    out_dir: &Path,
    version: &str,
    cfg: &DeployConfig,
) -> Result<()> {
    println!("==> Packaging {} …", out_dir.display());
    if out_dir.exists() {
        for entry in fs::read_dir(out_dir)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_dir() {
                fs::remove_dir_all(&p)?;
            } else {
                fs::remove_file(&p)?;
            }
        }
    } else {
        fs::create_dir_all(out_dir)?;
    }

    let build_ts = if matches!(profile, Profile::SelfHosted) {
        let input_wasm = paths.wasm_release_input();
        let hash = wasm::file_sha256(&input_wasm)?;
        hash[..10].to_string()
    } else {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string()
    };

    let (js_file, wasm_file, bindgen_name) = match profile {
        Profile::Crazygames | Profile::SiteDev => (
            PORTAL_JS.to_string(),
            PORTAL_WASM.to_string(),
            "sow_client".to_string(),
        ),
        Profile::SelfHosted => {
            let ts = build_ts.clone();
            (
                format!("sow_client_{ts}.js"),
                format!("sow_client_{ts}_bg.wasm"),
                format!("sow_client_{ts}"),
            )
        }
    };

    // Copy Astro website build to out_dir
    let sow_web_dir = paths.root.join("sow-web");
    let dist_shell = sow_web_dir.join("site");
    if dist_shell.is_dir() {
        copy_dir_all(&dist_shell, out_dir)?;
    }
    // Ensure CDN assets are included in the packaged build
    let cdn_dest = out_dir.join("assets/cdn");
    fs::create_dir_all(&cdn_dest)?;
    if paths.assets_cdn.is_dir() {
        copy_dir_all(&paths.assets_cdn, &cdn_dest)?;
    }

    // Ensure map files are included in the packaged build
    let maps_dest = out_dir.join("maps");
    fs::create_dir_all(&maps_dest)?;
    if paths.assets_maps.is_dir() {
        copy_dir_all(&paths.assets_maps, &maps_dest)?;
    }

    wasm::bindgen(paths, out_dir, &bindgen_name)?;
    copy_shell_extras(paths, out_dir)?;
    build_index_html(
        paths, out_dir, version, &js_file, &wasm_file, &build_ts, profile, cfg,
    )?;
    let play_index = out_dir.join("play/index.html");
    match profile {
        Profile::Crazygames => inject_crazygames(play_index, cfg)?,
        Profile::SiteDev => inject_site_embed(play_index, cfg)?,
        Profile::SelfHosted => {}
    }

    // Export i18n translation JSON strings for the web menu UI
    let locales_dir = out_dir.join("locales");
    fs::create_dir_all(&locales_dir)?;
    for lang in &[
        sow_i18n::Language::English,
        sow_i18n::Language::Spanish,
        sow_i18n::Language::French,
        sow_i18n::Language::German,
    ] {
        let strings = sow_i18n::get(*lang);
        let json = serde_json::to_string_pretty(strings)?;
        let name = match lang {
            sow_i18n::Language::English => "en",
            sow_i18n::Language::Spanish => "es",
            sow_i18n::Language::French => "fr",
            sow_i18n::Language::German => "de",
        };
        fs::write(locales_dir.join(format!("{name}.json")), json)?;
    }

    let js_path = out_dir.join(&js_file);
    let wasm_path = out_dir.join(&wasm_file);
    if !matches!(profile, Profile::SiteDev) {
        wasm::minify_js(&js_path)?;
        wasm::optimize_wasm(paths, &wasm_path)?;
    }

    let (js_load, wasm_load) = match profile {
        Profile::Crazygames => {
            wasm::brotli_file(&wasm_path)?;
            wasm::brotli_file(&js_path)?;
            wasm::require_brotli_sidecar(&wasm_path)?;
            wasm::require_brotli_sidecar(&js_path)?;
            fs::remove_file(&wasm_path)
                .with_context(|| format!("remove uncompressed wasm {}", wasm_path.display()))?;
            fs::remove_file(&js_path)
                .with_context(|| format!("remove uncompressed js {}", js_path.display()))?;
            let js_br = format!("{js_file}.br");
            let wasm_br = format!("{wasm_file}.br");
            patch_index_br(
                out_dir.join("play/index.html"),
                &js_file,
                &wasm_file,
                &js_br,
                &wasm_br,
            )?;
            (js_br, wasm_br)
        }
        Profile::SelfHosted => {
            wasm::brotli_file(&wasm_path)?;
            wasm::brotli_file(&js_path)?;
            wasm::require_brotli_sidecar(&wasm_path)?;
            wasm::require_brotli_sidecar(&js_path)?;
            write_sw(paths, out_dir, version, &js_file, &wasm_file, &build_ts)?;
            write_manifest(out_dir, version, &js_file, &wasm_file, &build_ts)?;
            (js_file.clone(), wasm_file.clone())
        }
        Profile::SiteDev => {
            write_manifest(out_dir, version, &js_file, &wasm_file, &build_ts)?;
            (js_file.clone(), wasm_file.clone())
        }
    };

    match profile {
        Profile::Crazygames | Profile::SiteDev => {
            stage_static_assets(paths, out_dir)?;
        }
        Profile::SelfHosted => {}
    }
    prune_querystring_artifacts(out_dir)?;
    verify_layout(out_dir, profile)?;

    println!("Load paths: {wasm_load}, {js_load}");
    Ok(())
}

fn copy_shell_extras(paths: &Paths, out: &Path) -> Result<()> {
    let fav = paths.shell.join("favicon_io");
    if fav.is_dir() {
        for e in fs::read_dir(&fav)? {
            let e = e?;
            let dest = out.join(e.file_name());
            if e.path().is_dir() {
                copy_dir_all(&e.path(), &dest)?;
            } else {
                fs::copy(e.path(), dest)?;
            }
        }
    }
    fs::copy(paths.shell.join("sow.svg"), out.join("sow.svg"))?;
    fs::copy(paths.shell.join("loader.js"), out.join("loader.js"))?;
    copy_dir_all(&paths.shell.join("sdk"), &out.join("sdk"))?;
    Ok(())
}

pub(crate) fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for e in fs::read_dir(src)? {
        let e = e?;
        let to = dst.join(e.file_name());
        if e.path().is_dir() {
            copy_dir_all(&e.path(), &to)?;
        } else {
            fs::copy(e.path(), to)?;
        }
    }
    Ok(())
}

/// Copy `src` tree into `dst`, skipping immediate child names in `exclude_top`.
fn copy_dir_excluding(src: &Path, dst: &Path, exclude_top: &[&str]) -> Result<()> {
    fs::create_dir_all(dst)?;
    for e in fs::read_dir(src)? {
        let e = e?;
        let name = e.file_name();
        let name = name.to_string_lossy();
        if e.path().is_dir() {
            if exclude_top.contains(&name.as_ref()) {
                continue;
            }
            copy_dir_all(&e.path(), &dst.join(e.file_name()))?;
        } else {
            fs::copy(e.path(), dst.join(e.file_name()))?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_index_html(
    paths: &Paths,
    out_dir: &Path,
    version: &str,
    js: &str,
    wasm: &str,
    build_ts: &str,
    profile: Profile,
    _cfg: &DeployConfig,
) -> Result<()> {
    let template = fs::read_to_string(paths.shell.join("index.html.template"))?;
    let assets_ui = "/assets/cdn/ui/".to_string();
    let html = template
        .replace("__VERSION__", version)
        .replace("./__JS_FILE__", "../__JS_FILE__")
        .replace("./__WASM_FILE__", "../__WASM_FILE__")
        .replace("__JS_FILE__", js)
        .replace("__WASM_FILE__", wasm)
        .replace("__BUILD_TS__", build_ts)
        .replace("__ASSETS_UI_BASE__", &assets_ui)
        .replace("href=\"./sow.svg\"", "href=\"../sow.svg\"")
        .replace("href=\"./favicon.ico\"", "href=\"../favicon.ico\"")
        .replace("src=\"./loader.js\"", "src=\"../loader.js\"")
        .replace("src=\"./sdk/store_portals.js\"", "src=\"../sdk/store_portals.js\"")
        .replace("register('./sw.js', { scope: './' })", "register('../sw.js', { scope: '../' })");
    let index = out_dir.join("play/index.html");
    if let Some(parent) = index.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&index, &html)?;
    inline_loader(paths, &index)?;
    if matches!(profile, Profile::Crazygames) {
        // keep PORTAL slots for inject_crazygames
    }
    Ok(())
}

fn inline_loader(paths: &Paths, html_path: &Path) -> Result<()> {
    let loader = fs::read_to_string(paths.shell.join("loader.js"))?;
    let js = loader.replace("</script>", "<\\/script>");
    let mut html = fs::read_to_string(html_path)?;
    const MARKER: &str = "/* __INLINE_LOADER_JS__ */";
    if html.contains(MARKER) {
        html = html.replacen(MARKER, &js, 1);
    } else if html.contains(r#"<script src="./loader.js"></script>"#) {
        html = html.replacen(
            r#"<script src="./loader.js"></script>"#,
            &format!("<script>\n{js}\n</script>"),
            1,
        );
    } else if html.contains(r#"<script src="../loader.js"></script>"#) {
        html = html.replacen(
            r#"<script src="../loader.js"></script>"#,
            &format!("<script>\n{js}\n</script>"),
            1,
        );
    } else {
        bail!("index.html: no loader injection point");
    }
    fs::write(html_path, html)?;
    Ok(())
}

fn inject_portal_slots(html_path: PathBuf, sdk_line: Option<&str>, boot_line: &str) -> Result<()> {
    let mut lines: Vec<String> = fs::read_to_string(&html_path)?
        .lines()
        .map(String::from)
        .collect();
    let mut sdk_ok = sdk_line.is_none();
    let mut boot_ok = false;
    for line in &mut lines {
        if line.contains("PORTAL_SDK_SLOT") {
            if let Some(sdk) = sdk_line {
                *line = sdk.to_string();
            }
            sdk_ok = true;
        } else if line.contains("PORTAL_BOOT_SLOT") {
            *line = boot_line.to_string();
            boot_ok = true;
        }
    }
    if !sdk_ok || !boot_ok {
        bail!(
            "inject_portal_slots: missing portal slots in {}",
            html_path.display()
        );
    }
    fs::write(html_path, lines.join("\n") + "\n")?;
    Ok(())
}

fn inject_crazygames(html_path: PathBuf, cfg: &DeployConfig) -> Result<()> {
    let sdk = r#"    <script src="https://sdk.crazygames.com/crazygames-sdk-v3.js"></script>"#;
    let site_url = cfg.site_url();
    let boot = format!(
        r#"        window.SOW_PORTAL = "crazygames"; window.SOW_WS_URL = "{ws}"; window.SOW_MAPS_URL = "{site_url}/maps";"#,
        ws = cfg.ws_url(&cfg.site_origin),
        site_url = site_url,
    );
    inject_portal_slots(html_path, Some(sdk), &boot)
}

fn inject_site_embed(html_path: PathBuf, cfg: &DeployConfig) -> Result<()> {
    let site_url = cfg.site_url();
    let boot = format!(
        r#"        window.SOW_PORTAL = "site"; window.SOW_WS_URL = "{ws}"; window.SOW_MAPS_URL = "{site_url}/maps"; window.SOW_ASSETS_URL = "/assets";"#,
        ws = cfg.ws_url(&cfg.site_origin),
        site_url = site_url,
    );
    inject_portal_slots(html_path, None, &boot)
}

fn patch_index_br(index: PathBuf, js: &str, wasm: &str, js_br: &str, wasm_br: &str) -> Result<()> {
    let mut html = fs::read_to_string(&index)?;
    html = html.replace(&format!("./{js}"), &format!("./{js_br}"));
    html = html.replace(&format!("./{wasm}"), &format!("./{wasm_br}"));
    fs::write(index, html)?;
    Ok(())
}

fn write_sw(
    paths: &Paths,
    out: &Path,
    version: &str,
    js: &str,
    wasm: &str,
    ts: &str,
) -> Result<()> {
    let tpl = fs::read_to_string(paths.shell.join("sw.js.template"))?;
    let sw = tpl
        .replace("__VERSION__", version)
        .replace("__JS_FILE__", js)
        .replace("__WASM_FILE__", wasm)
        .replace("__BUILD_TS__", ts);
    fs::write(out.join("sw.js"), sw)?;
    Ok(())
}

fn write_manifest(out: &Path, version: &str, js: &str, wasm: &str, ts: &str) -> Result<()> {
    let json =
        format!(r#"{{"js":"{js}","wasm":"{wasm}","build_ts":"{ts}","version":"{version}"}}"#,);
    fs::write(out.join("game-manifest.json"), json)?;
    Ok(())
}

fn stage_static_assets(paths: &Paths, out: &Path) -> Result<()> {
    let dest = out.join("assets/static");
    if dest.exists() {
        fs::remove_dir_all(&dest)?;
    }
    fs::create_dir_all(dest.parent().unwrap())?;
    println!("==> Staging assets/static (no maps/) → {}", dest.display());
    copy_dir_excluding(&paths.assets_static, &dest, &["maps"])?;
    println!("✅ Staging assets/static finished");
    Ok(())
}

fn prune_querystring_artifacts(root: &Path) -> Result<()> {
    let mut removed = 0u32;
    for entry in WalkDir::new(root) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let name = entry.file_name().to_string_lossy();
            if name.contains("?v=") {
                fs::remove_file(entry.path())?;
                removed += 1;
            }
        }
    }
    if removed > 0 {
        println!("Removed {removed} querystring artifact(s)");
    }
    Ok(())
}
