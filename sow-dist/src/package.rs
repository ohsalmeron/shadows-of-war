use crate::paths::{Paths, PORTAL_JS, PORTAL_WASM};
use crate::process;
use crate::wasm;
use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

#[derive(Clone, Copy, Debug)]
pub enum Profile {
    SelfHosted,
    Crazygames,
}

pub fn build(paths: &Paths, profile: Profile, out_dir: &Path, version: &str) -> Result<()> {
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

    let build_ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    let (js_file, wasm_file, bindgen_name) = match profile {
        Profile::Crazygames => (
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

    wasm::bindgen(paths, out_dir, &bindgen_name)?;
    copy_shell_extras(paths, out_dir)?;
    build_index_html(
        paths,
        out_dir,
        version,
        &js_file,
        &wasm_file,
        &build_ts,
        profile,
    )?;
    if matches!(profile, Profile::Crazygames) {
        inject_crazygames(out_dir.join("index.html"))?;
    }

    let js_path = out_dir.join(&js_file);
    let wasm_path = out_dir.join(&wasm_file);
    wasm::minify_js(&js_path)?;
    wasm::optimize_wasm(paths, &wasm_path)?;

    let (js_load, wasm_load) = match profile {
        Profile::Crazygames => {
            wasm::brotli_file(&wasm_path)?;
            wasm::brotli_file(&js_path)?;
            fs::remove_file(&wasm_path)?;
            fs::remove_file(&js_path)?;
            let js_br = format!("{js_file}.br");
            let wasm_br = format!("{wasm_file}.br");
            patch_index_br(out_dir.join("index.html"), &js_file, &wasm_file, &js_br, &wasm_br)?;
            (js_br, wasm_br)
        }
        Profile::SelfHosted => {
            wasm::brotli_file(&wasm_path)?;
            wasm::brotli_file(&js_path)?;
            write_sw(paths, out_dir, version, &js_file, &wasm_file, &build_ts)?;
            write_manifest(out_dir, version, &js_file, &wasm_file, &build_ts)?;
            (js_file.clone(), wasm_file.clone())
        }
    };

    stage_static_assets(paths, out_dir)?;
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
    copy_dir_all(&paths.shell.join("sdk"), &out.join("sdk"))?;
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
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

fn build_index_html(
    paths: &Paths,
    out_dir: &Path,
    version: &str,
    js: &str,
    wasm: &str,
    build_ts: &str,
    profile: Profile,
) -> Result<()> {
    let template = fs::read_to_string(paths.shell.join("index.html.template"))?;
    let assets_ui = "https://shadowsofwar.io/assets/cdn/ui/";
    let html = template
        .replace("__VERSION__", version)
        .replace("__JS_FILE__", js)
        .replace("__WASM_FILE__", wasm)
        .replace("__BUILD_TS__", build_ts)
        .replace("__ASSETS_UI_BASE__", assets_ui);
    let index = out_dir.join("index.html");
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
    } else {
        bail!("index.html: no loader injection point");
    }
    fs::write(html_path, html)?;
    Ok(())
}

fn inject_crazygames(html_path: PathBuf) -> Result<()> {
    let sdk = r#"    <script src="https://sdk.crazygames.com/crazygames-sdk-v3.js"></script>"#;
    let boot = r#"        window.SOW_PORTAL = "crazygames"; window.SOW_WS_URL = "wss://shadowsofwar.io/ws/"; window.SOW_MAPS_URL = "https://shadowsofwar.io/maps";"#;
    let mut lines: Vec<String> = fs::read_to_string(&html_path)?
        .lines()
        .map(String::from)
        .collect();
    let mut sdk_ok = false;
    let mut boot_ok = false;
    for line in &mut lines {
        if line.contains("PORTAL_SDK_SLOT") {
            *line = sdk.to_string();
            sdk_ok = true;
        } else if line.contains("PORTAL_BOOT_SLOT") {
            *line = boot.to_string();
            boot_ok = true;
        }
    }
    if !sdk_ok || !boot_ok {
        bail!("inject_crazygames: missing portal slots");
    }
    fs::write(html_path, lines.join("\n") + "\n")?;
    Ok(())
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
    let json = format!(
        r#"{{"js":"{js}","wasm":"{wasm}","build_ts":"{ts}","version":"{version}"}}"#,
    );
    fs::write(out.join("game-manifest.json"), json)?;
    Ok(())
}

fn stage_static_assets(paths: &Paths, out: &Path) -> Result<()> {
    let dest = out.join("assets/static");
    if dest.exists() {
        fs::remove_dir_all(&dest)?;
    }
    fs::create_dir_all(dest.parent().unwrap())?;
    println!("==> Staging assets/static → {}", dest.display());
    let src = format!("{}/", paths.assets_static.display());
    let dst = format!("{}/", dest.display());
    process::run("rsync", &["-a", &src, &dst], None)?;
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

pub fn verify_layout(dir: &Path, profile: Profile) -> Result<()> {
    if dir.join("assets/cdn").exists() {
        bail!("{} must not contain assets/cdn/ (CDN is remote only)", dir.display());
    }
    let font = dir.join("assets/static/fonts/JockeyOne-Regular.ttf");
    if !font.is_file() {
        bail!("{} missing assets/static/fonts/JockeyOne-Regular.ttf", dir.display());
    }
    match profile {
        Profile::Crazygames => {
            if !dir.join(format!("{PORTAL_WASM}.br")).is_file() {
                bail!("missing {PORTAL_WASM}.br");
            }
            if !dir.join(format!("{PORTAL_JS}.br")).is_file() {
                bail!("missing {PORTAL_JS}.br");
            }
            if dir.join(PORTAL_WASM).is_file() || dir.join(PORTAL_JS).is_file() {
                bail!("portal dist must be .br only");
            }
        }
        Profile::SelfHosted => {
            let has_wasm = fs::read_dir(dir)?
                .filter_map(|e| e.ok())
                .any(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .ends_with("_bg.wasm")
                        && !e.file_name().to_string_lossy().ends_with(".br")
                });
            if !has_wasm {
                bail!("missing raw *_bg.wasm");
            }
        }
    }
    println!("✅ Dist layout OK ({})", dir.display());
    Ok(())
}
