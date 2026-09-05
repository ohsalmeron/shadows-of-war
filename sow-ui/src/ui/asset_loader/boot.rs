use super::*;

#[cfg(not(target_arch = "wasm32"))]
use super::retry::decode_and_upload_webp;

impl AssetLoader {
    pub fn ui_splash_ready(&self) -> bool {
        self.ui_loader_empty.is_some()
            && self.ui_loader_full.is_some()
            && self.splash_desktop.is_some()
            && self.splash_mobile.is_some()
    }

    pub fn request_boot_ui_fetch(&mut self, kind: UiSplashTexture) {
        if kind.loaded_in(self) || self.boot_ui_in_flight.contains(&kind) {
            return;
        }
        if self.boot_ui_fetch_pending.contains(&kind) {
            return;
        }
        self.boot_ui_fetch_pending.push(kind);
    }

    pub fn take_next_boot_ui_fetch_pending(&mut self) -> Option<UiSplashTexture> {
        if self.boot_ui_in_flight.len() >= MAX_BOOT_UI_FETCHES_IN_FLIGHT {
            return None;
        }
        let kind = self.boot_ui_fetch_pending.first().copied()?;
        self.boot_ui_fetch_pending.remove(0);
        self.boot_ui_in_flight.insert(kind);
        Some(kind)
    }

    pub fn note_boot_ui_fetch_failed(&mut self, kind: UiSplashTexture) {
        self.boot_ui_in_flight.remove(&kind);
    }

    pub fn ingest_boot_ui_webp_bytes(
        &mut self,
        ctx: &egui::Context,
        kind: UiSplashTexture,
        bytes: &[u8],
    ) -> Result<(), String> {
        self.boot_ui_in_flight.remove(&kind);
        let mut image =
            image::load_from_memory(bytes).map_err(|e| format!("decode {:?}: {e}", kind))?;
        if image.width() > 2048 || image.height() > 2048 {
            image = image.resize(2048, 2048, image::imageops::FilterType::Triangle);
        }
        let image_rgba = image.to_rgba8();
        let width = image_rgba.width();
        let height = image_rgba.height();
        let rgba = image_rgba.as_raw();
        if !self.ingest_ui_splash_texture(ctx, kind, width, height, rgba) {
            return Err(format!("ingest {:?} size mismatch", kind));
        }
        Ok(())
    }

    pub fn ingest_ui_splash_texture(
        &mut self,
        ctx: &egui::Context,
        kind: UiSplashTexture,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> bool {
        let expected = (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4);
        if rgba.len() != expected {
            log::warn!(
                "ui splash texture {:?} size mismatch: got {} expected {}",
                kind,
                rgba.len(),
                expected
            );
            return false;
        }

        let color_image = egui::ColorImage::from_rgba_unmultiplied([width as _, height as _], rgba);
        let texture = ctx.load_texture(
            match kind {
                UiSplashTexture::LoaderEmpty => "ui_loader_empty",
                UiSplashTexture::LoaderFull => "ui_loader_full",
                UiSplashTexture::SplashDesktop => "sow_splash_desktop",
                UiSplashTexture::SplashMobile => "sow_splash_mobile",
            },
            color_image,
            egui::TextureOptions::LINEAR,
        );

        match kind {
            UiSplashTexture::LoaderEmpty => self.ui_loader_empty = Some(texture),
            UiSplashTexture::LoaderFull => self.ui_loader_full = Some(texture),
            UiSplashTexture::SplashDesktop => self.splash_desktop = Some(texture),
            UiSplashTexture::SplashMobile => self.splash_mobile = Some(texture),
        }
        true
    }

    pub fn boot_leader_ready(&self, leader: Leader, mobile: bool) -> bool {
        let key = LeaderPortraitKey { leader, mobile };
        self.leader_portrait_ready(leader, mobile) || self.leader_retry_state.contains_key(&key)
    }

    pub fn ensure_ui_assets_loaded(&mut self, ctx: &egui::Context) {
        if !self
            .thumbnails
            .contains_key(sow_core::maps::DEFAULT_MAP_KEY)
        {
            let bytes = sow_ui_kit::repo_asset_bytes!("maps/world/thumbnail.webp");
            if let Some(color_image) =
                crate::ui::map_texture::color_image_from_map_thumbnail_bytes(bytes)
            {
                let key = sow_core::maps::DEFAULT_MAP_KEY.to_string();
                let texture = ctx.load_texture(&key, color_image, egui::TextureOptions::LINEAR);
                self.thumbnails.insert(key, texture);
            }
        }

        if self.ui_splash_ready() {
            return;
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = ctx;
            for kind in UiSplashTexture::ALL {
                self.request_boot_ui_fetch(kind);
            }
            return;
        }

        // Store-style Apple bundles must be self-contained. Keep the boot art
        // embedded for macOS just like iOS; sandboxed apps cannot read from
        // the repository-relative path used by the legacy desktop launcher.
        #[cfg(all(
            not(target_arch = "wasm32"),
            any(target_os = "ios", target_os = "macos")
        ))]
        {
            let load_image = |name: &str, bytes: &[u8]| decode_and_upload_webp(ctx, name, bytes);

            self.ui_loader_empty = Some(load_image(
                "ui_loader_empty",
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../assets/shell/loader/loader_empty.webp"
                )),
            ));
            self.ui_loader_full = Some(load_image(
                "ui_loader_full",
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../assets/shell/loader/loader_full.webp"
                )),
            ));
            self.splash_desktop = Some(load_image(
                "sow_splash_desktop",
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../assets/shell/loader/sow-splash-desktop.webp"
                )),
            ));
            self.splash_mobile = Some(load_image(
                "sow_splash_mobile",
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../assets/shell/loader/sow-splash-mobile.webp"
                )),
            ));
        }

        #[cfg(all(
            not(target_arch = "wasm32"),
            not(any(target_os = "ios", target_os = "macos"))
        ))]
        {
            fn read_ui_webp(filename: &str) -> Vec<u8> {
                use std::path::Path;
                let base = Path::new(env!("CARGO_MANIFEST_DIR"));
                let path = base.join("../assets/shell/loader").join(filename);
                std::fs::read(&path).unwrap_or_else(|e| {
                    panic!("missing UI asset {filename} in assets/shell/loader: {e}")
                })
            }

            let load_image = |name: &str, bytes: &[u8]| decode_and_upload_webp(ctx, name, bytes);

            let loader_empty = read_ui_webp("loader_empty.webp");
            let loader_full = read_ui_webp("loader_full.webp");
            let splash_desktop = read_ui_webp("sow-splash-desktop.webp");
            let splash_mobile = read_ui_webp("sow-splash-mobile.webp");

            self.ui_loader_empty = Some(load_image("ui_loader_empty", &loader_empty));
            self.ui_loader_full = Some(load_image("ui_loader_full", &loader_full));
            self.splash_desktop = Some(load_image("sow_splash_desktop", &splash_desktop));
            self.splash_mobile = Some(load_image("sow_splash_mobile", &splash_mobile));
        }
    }

    /// Preload menu hero portraits for `leader` (boot splash), not a fixed Caesar default.
    pub fn ensure_boot_leader_loaded(&mut self, ctx: &egui::Context, leader: Leader) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::path::Path;
            let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/shell/leaders");
            for mobile in [false, true] {
                let key = LeaderPortraitKey { leader, mobile };
                if self.leader_portrait_loaded(key) {
                    continue;
                }
                let filename = Self::leader_portrait_filename(key);
                let path = base.join(&filename);
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                if let Ok(color_image) = Self::decode_leader_portrait_bytes(&bytes) {
                    let name_lower = Self::leader_slug(leader);
                    let tex_name = if mobile {
                        format!("leader_{name_lower}_mobile")
                    } else {
                        format!("leader_{name_lower}_desktop")
                    };
                    let texture =
                        ctx.load_texture(&tex_name, color_image, egui::TextureOptions::LINEAR);
                    if mobile {
                        self.leader_mobile_images.insert(leader, texture);
                    } else {
                        self.leader_desktop_images.insert(leader, texture);
                    }
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = ctx;
            self.request_leader_portrait_priority(leader, false);
            self.request_leader_portrait_priority(leader, true);
        }
    }

    /// Load the complete leader art set used by the native store. The menu hero only needs
    /// the selected leader, while a store grid must never show a text placeholder for the
    /// other offers.
    pub fn ensure_store_leader_portraits_loaded(&mut self, ctx: &egui::Context) {
        #[cfg(all(
            not(target_arch = "wasm32"),
            any(target_os = "ios", target_os = "macos")
        ))]
        {
            let assets: &[(Leader, &[u8])] = &[
                (
                    Leader::Caesar,
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../assets/shell/leaders/caesar_desktop.webp"
                    )),
                ),
                (
                    Leader::Cleopatra,
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../assets/shell/leaders/cleopatra_desktop.webp"
                    )),
                ),
                (
                    Leader::Ragnar,
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../assets/shell/leaders/ragnar_desktop.webp"
                    )),
                ),
                (
                    Leader::SunTzu,
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../assets/shell/leaders/sun_tzu_desktop.webp"
                    )),
                ),
                (
                    Leader::Alexander,
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../assets/shell/leaders/alexander_desktop.webp"
                    )),
                ),
                (
                    Leader::GenghisKhan,
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../assets/shell/leaders/genghis_khan_desktop.webp"
                    )),
                ),
                (
                    Leader::RichardTheLionheart,
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../assets/shell/leaders/richard_the_lionheart_desktop.webp"
                    )),
                ),
                (
                    Leader::Vercingetorix,
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../assets/shell/leaders/vercingetorix_desktop.webp"
                    )),
                ),
                (
                    Leader::Boudica,
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../assets/shell/leaders/boudica_desktop.webp"
                    )),
                ),
                (
                    Leader::LadySixSky,
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../assets/shell/leaders/lady_six_sky_desktop.webp"
                    )),
                ),
                (
                    Leader::Leonidas,
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../assets/shell/leaders/leonidas_desktop.webp"
                    )),
                ),
                (
                    Leader::Napoleon,
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../assets/shell/leaders/napoleon_desktop.webp"
                    )),
                ),
            ];
            for &(leader, bytes) in assets {
                if self.leader_desktop_images.contains_key(&leader) {
                    continue;
                }
                if let Ok(color_image) = Self::decode_leader_portrait_bytes(bytes) {
                    let texture = ctx.load_texture(
                        format!("store_leader_{}", Self::leader_slug(leader)),
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.leader_desktop_images.insert(leader, texture);
                }
            }
        }

        #[cfg(all(
            not(target_arch = "wasm32"),
            not(any(target_os = "ios", target_os = "macos"))
        ))]
        {
            use std::path::Path;
            let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/shell/leaders");
            for leader in Leader::ALL {
                if self.leader_desktop_images.contains_key(&leader) {
                    continue;
                }
                let key = LeaderPortraitKey {
                    leader,
                    mobile: false,
                };
                let path = base.join(Self::leader_portrait_filename(key));
                let Ok(bytes) = std::fs::read(path) else {
                    continue;
                };
                if let Ok(color_image) = Self::decode_leader_portrait_bytes(&bytes) {
                    let texture = ctx.load_texture(
                        format!("store_leader_{}", Self::leader_slug(leader)),
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.leader_desktop_images.insert(leader, texture);
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        for leader in Leader::ALL {
            self.request_leader_portrait(leader, false);
        }
    }
}
