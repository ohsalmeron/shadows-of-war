use super::build::copy_dir_all;
use super::Profile;
use crate::assets::UI_FONT_FILE;
use crate::paths::{Paths, PORTAL_JS, PORTAL_WASM};
use anyhow::{bail, Result};
use std::fs;
use std::path::Path;

pub fn verify_layout(dir: &Path, profile: Profile) -> Result<()> {
    match profile {
        Profile::Crazygames | Profile::SiteDev => {
            let font = dir.join("assets/static/fonts").join(UI_FONT_FILE);
            if !font.is_file() {
                bail!(
                    "{} missing assets/static/fonts/{UI_FONT_FILE}",
                    dir.display()
                );
            }
            if dir.join("assets/static/maps").exists() {
                bail!(
                    "{} must not bundle assets/static/maps/ (offline map is embedded in WASM)",
                    dir.display()
                );
            }
        }
        Profile::SelfHosted => {}
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
            let mut wasm_name = None;
            let mut js_name = None;
            for e in fs::read_dir(dir)? {
                let e = e?;
                let name = e.file_name().to_string_lossy().into_owned();
                if name.ends_with("_bg.wasm") && !name.ends_with(".br") {
                    wasm_name = Some(name);
                } else if name.starts_with("sow_client_")
                    && name.ends_with(".js")
                    && !name.ends_with(".br")
                {
                    js_name = Some(name);
                }
            }
            let Some(wasm_name) = wasm_name else {
                bail!("missing raw *_bg.wasm");
            };
            let Some(js_name) = js_name else {
                bail!("missing raw sow_client_*.js");
            };
            if !dir.join(format!("{wasm_name}.br")).is_file() {
                bail!("missing brotli sidecar {wasm_name}.br");
            }
            if !dir.join(format!("{js_name}.br")).is_file() {
                bail!("missing brotli sidecar {js_name}.br");
            }
        }
        Profile::SiteDev => {
            if !dir.join(PORTAL_WASM).is_file() {
                bail!("missing {PORTAL_WASM}");
            }
            if !dir.join(PORTAL_JS).is_file() {
                bail!("missing {PORTAL_JS}");
            }
        }
    }
    println!("✅ Dist layout OK ({})", dir.display());
    Ok(())
}

/// Merge marketing site + game shell into `dist/site-dev/www/` (iframe → `/game/`).
pub fn stage_site_www(paths: &Paths) -> Result<()> {
    let www = &paths.dist_site_dev_www;
    let game = &paths.dist_site_dev_game;
    if !game.join("index.html").is_file() {
        bail!(
            "missing {} — run package::build(SiteDev) first",
            game.join("index.html").display()
        );
    }
    if www.exists() {
        fs::remove_dir_all(www)?;
    }
    fs::create_dir_all(www)?;
    println!("==> Staging site → {}", www.display());
    copy_dir_all(&paths.site_web, www)?;
    println!("✅ Staging site finished");
    println!("==> Staging game shell → {}", www.join("game").display());
    copy_dir_all(game, &www.join("game"))?;
    println!("✅ Staging game shell finished");
    println!("==> Staging game shell to root → {}", www.display());
    copy_dir_all(game, www)?;
    println!("✅ Staging game shell to root finished");
    Ok(())
}
