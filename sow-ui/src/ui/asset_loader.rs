use egui::TextureHandle;
use sow_core::player::Leader;
use std::collections::{HashMap, HashSet};

/// Desktop vs mobile leader portrait variant (for streamed assets on wasm32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LeaderPortraitKey {
    pub leader: Leader,
    pub mobile: bool,
}

pub struct AssetLoader {
    /// Dynamic map list from `catalog.bin` (fetched at runtime).
    pub map_catalog: Option<Vec<sow_core::maps::MapCatalogEntry>>,
    pub catalog_in_flight: bool,
    /// Fully downloaded + cached map binary data (compressed .br bytes)
    pub maps: HashMap<String, Vec<u8>>,
    /// Maps currently being fetched
    pub maps_in_flight: HashSet<String>,
    /// Decoded thumbnail textures ready for egui
    pub thumbnails: HashMap<String, TextureHandle>,
    /// Thumbnails currently being fetched
    pub thumbnails_in_flight: HashSet<String>,
    /// Last fetch/decode failure per map key (for main-menu debug display).
    pub thumbnail_errors: HashMap<String, String>,
    /// Queued thumbnail keys; drained by sow-client each frame.
    pub thumbnails_fetch_pending: Vec<String>,
    /// Pre-loaded avatar textures
    pub avatars: HashMap<sow_core::player::Leader, TextureHandle>,
    pub avatar_fallback: Option<TextureHandle>,
    pub ui_loader_empty: Option<TextureHandle>,
    pub ui_loader_full: Option<TextureHandle>,
    pub leader_desktop_images: HashMap<sow_core::player::Leader, TextureHandle>,
    pub leader_mobile_images: HashMap<sow_core::player::Leader, TextureHandle>,
    pub splash_desktop: Option<TextureHandle>,
    pub splash_mobile: Option<TextureHandle>,
    pub hud_icons: HashMap<crate::ui::hud::icons::HudIcon, TextureHandle>,
    /// Queued leader portrait fetches (wasm32); drained by sow-client network layer.
    pub leaders_fetch_pending: Vec<LeaderPortraitKey>,
    pub leaders_in_flight: HashSet<LeaderPortraitKey>,
}

impl Default for AssetLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Splash / loader bar textures used by the loading screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiSplashTexture {
    LoaderEmpty,
    LoaderFull,
    SplashDesktop,
    SplashMobile,
}

impl AssetLoader {
    /// Normalize map display names to folder keys (e.g. "Bering Strait" -> "beringstrait").
    pub fn map_key(name: &str) -> String {
        sow_core::maps::map_key(name)
    }

    pub fn new() -> Self {
        Self {
            map_catalog: None,
            catalog_in_flight: false,
            maps: HashMap::new(),
            maps_in_flight: HashSet::new(),
            thumbnails: HashMap::new(),
            thumbnails_in_flight: HashSet::new(),
            thumbnail_errors: HashMap::new(),
            thumbnails_fetch_pending: Vec::new(),
            avatars: HashMap::new(),
            avatar_fallback: None,
            ui_loader_empty: None,
            ui_loader_full: None,
            leader_desktop_images: HashMap::new(),
            leader_mobile_images: HashMap::new(),
            splash_desktop: None,
            splash_mobile: None,
            hud_icons: HashMap::new(),
            leaders_fetch_pending: Vec::new(),
            leaders_in_flight: HashSet::new(),
        }
    }

    fn leader_portrait_loaded(&self, key: LeaderPortraitKey) -> bool {
        if key.mobile {
            self.leader_mobile_images.contains_key(&key.leader)
        } else {
            self.leader_desktop_images.contains_key(&key.leader)
        }
    }

    /// Queue a leader portrait download on wasm32 (no-op when already loaded or in flight).
    pub fn request_leader_portrait(&mut self, leader: Leader, mobile: bool) {
        let key = LeaderPortraitKey { leader, mobile };
        if self.leader_portrait_loaded(key) || self.leaders_in_flight.contains(&key) {
            return;
        }
        if !self
            .leaders_fetch_pending
            .iter()
            .any(|pending| *pending == key)
        {
            self.leaders_fetch_pending.push(key);
        }
    }

    /// Drain pending leader portrait requests for the client to fetch over HTTP.
    pub fn drain_leader_fetch_pending(&mut self) -> Vec<LeaderPortraitKey> {
        let pending: Vec<_> = self.leaders_fetch_pending.drain(..).collect();
        for key in &pending {
            self.leaders_in_flight.insert(*key);
        }
        pending
    }

    pub fn leader_portrait_filename(key: LeaderPortraitKey) -> String {
        let name_lower = key.leader.name().to_lowercase().replace(' ', "_");
        let form = if key.mobile { "mobile" } else { "desktop" };
        format!("{name_lower}_{form}.webp")
    }

    pub fn ingest_leader_portrait(
        &mut self,
        ctx: &egui::Context,
        leader: Leader,
        mobile: bool,
        bytes: &[u8],
    ) {
        let key = LeaderPortraitKey { leader, mobile };
        self.leaders_in_flight.remove(&key);

        let mut image = match image::load_from_memory(bytes) {
            Ok(img) => img,
            Err(e) => {
                log::warn!(
                    "Failed to decode leader portrait {:?} mobile={}: {}",
                    leader,
                    mobile,
                    e
                );
                return;
            }
        };
        if image.width() > 2048 || image.height() > 2048 {
            image = image.resize(2048, 2048, image::imageops::FilterType::Triangle);
        }
        let image_rgba = image.to_rgba8();
        let size = [image_rgba.width() as _, image_rgba.height() as _];
        let pixels = image_rgba.as_flat_samples();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
        let name_lower = leader.name().to_lowercase().replace(' ', "_");
        let tex_name = if mobile {
            format!("leader_{name_lower}_mobile")
        } else {
            format!("leader_{name_lower}_desktop")
        };
        let texture = ctx.load_texture(tex_name, color_image, egui::TextureOptions::LINEAR);
        if mobile {
            self.leader_mobile_images.insert(leader, texture);
        } else {
            self.leader_desktop_images.insert(leader, texture);
        }
    }

    pub fn has_map(&self, map_name: &str) -> bool {
        let key = Self::map_key(map_name);
        self.maps.contains_key(&key) || sow_core::maps::map_payload_available(&key)
    }

    pub fn take_map(&mut self, map_name: &str) -> Option<Vec<u8>> {
        let key = Self::map_key(map_name);
        let cached = self.maps.remove(&key);
        sow_core::maps::load_map_br_payload(&key, cached)
    }

    pub fn thumbnail(&self, map_name: &str) -> Option<&TextureHandle> {
        self.thumbnails.get(&Self::map_key(map_name))
    }

    pub fn thumbnail_in_flight(&self, map_name: &str) -> bool {
        self.thumbnails_in_flight.contains(&Self::map_key(map_name))
    }

    pub fn thumbnail_error(&self, map_name: &str) -> Option<&str> {
        self.thumbnail_errors
            .get(&Self::map_key(map_name))
            .map(String::as_str)
    }

    /// Insert a decoded thumbnail texture (always keyed with [`map_key`]).
    pub fn ingest_thumbnail(
        &mut self,
        ctx: &egui::Context,
        map_name: &str,
        bytes: &[u8],
    ) -> Result<(), String> {
        let key = Self::map_key(map_name);
        let color_image = crate::ui::map_texture::color_image_from_map_thumbnail_bytes(bytes)
            .ok_or_else(|| "decode failed (invalid or unsupported image)".to_string())?;
        let texture = ctx.load_texture(
            format!("map_thumb_{key}"),
            color_image,
            egui::TextureOptions::LINEAR,
        );
        self.thumbnails.insert(key.clone(), texture);
        self.thumbnails_in_flight.remove(&key);
        self.thumbnail_errors.remove(&key);
        Ok(())
    }

    pub fn note_thumbnail_failure(&mut self, map_name: &str, reason: impl Into<String>) {
        let key = Self::map_key(map_name);
        self.thumbnails_in_flight.remove(&key);
        self.thumbnail_errors.insert(key, reason.into());
    }

    /// Queue a thumbnail fetch when missing (idempotent).
    pub fn request_thumbnail(&mut self, map_name: &str) {
        let key = Self::map_key(map_name);
        if self.thumbnails.contains_key(&key)
            || self.thumbnails_in_flight.contains(&key)
            || self
                .thumbnails_fetch_pending
                .iter()
                .any(|pending| pending == &key)
        {
            return;
        }
        self.thumbnails_in_flight.insert(key.clone());
        self.thumbnail_errors.remove(&key);
        self.thumbnails_fetch_pending.push(key);
    }

    pub fn drain_thumbnail_fetch_pending(&mut self) -> Vec<String> {
        self.thumbnails_fetch_pending.drain(..).collect()
    }

    pub fn get_assets_to_fetch(
        &mut self,
        lobbies: &[sow_core::protocol::LobbyInfo],
    ) -> (Vec<String>, Vec<String>) {
        let mut maps_to_fetch = Vec::new();

        let mut unique_maps = HashSet::new();
        for l in lobbies {
            unique_maps.insert(l.map_name.clone());
        }

        // Thumbnails: queue missing ones; sow-client drains via poll_thumbnail_fetches.
        for map_name in &unique_maps {
            self.request_thumbnail(map_name);
        }
        let thumbs_to_fetch = Vec::new();

        // Maps: only fetch if no other map is currently downloading to prevent network congestion
        if self.maps_in_flight.is_empty() {
            let mut target_map = None;
            if let Some(primary) = crate::ui::main_menu::primary_lobby_for_browser(lobbies) {
                let key = Self::map_key(&primary.map_name);
                if !self.has_map(&key) {
                    target_map = Some(key);
                }
            }
            if target_map.is_none() {
                for map_name in &unique_maps {
                    let key = Self::map_key(map_name);
                    if !self.has_map(&key) {
                        target_map = Some(key);
                        break;
                    }
                }
            }

            if let Some(m) = target_map {
                self.maps_in_flight.insert(m.clone());
                maps_to_fetch.push(m);
            }
        }

        (thumbs_to_fetch, maps_to_fetch)
    }

    pub fn flush_except(&mut self, keep: &[String]) {
        let keep_set: HashSet<&String> = keep.iter().collect();
        self.maps.retain(|k, _| keep_set.contains(k));
    }

    pub fn ensure_avatars_loaded(&mut self, ctx: &egui::Context) {
        if !self.avatars.is_empty() {
            return;
        }

        let load_image = |name: &str, bytes: &[u8]| -> TextureHandle {
            let image = image::load_from_memory(bytes)
                .expect("Failed to load avatar")
                .to_rgba8();
            let size = [image.width() as _, image.height() as _];
            let pixels = image.as_flat_samples();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR)
        };

        for &leader in &sow_core::player::Leader::ALL {
            let avatar_bytes = match leader {
                sow_core::player::Leader::Caesar => {
                    include_bytes!("../../assets/avatars/caesar.webp").as_slice()
                }
                sow_core::player::Leader::Cleopatra => {
                    include_bytes!("../../assets/avatars/cleopatra.webp").as_slice()
                }
                sow_core::player::Leader::Ragnar => {
                    include_bytes!("../../assets/avatars/ragnar.webp").as_slice()
                }
                sow_core::player::Leader::SunTzu => {
                    include_bytes!("../../assets/avatars/sun_tzu.webp").as_slice()
                }
                sow_core::player::Leader::Alexander => {
                    include_bytes!("../../assets/avatars/alexander.webp").as_slice()
                }
                sow_core::player::Leader::GenghisKhan => {
                    include_bytes!("../../assets/avatars/genghis_khan.webp").as_slice()
                }
                sow_core::player::Leader::RichardTheLionheart => {
                    include_bytes!("../../assets/avatars/richard_the_lionheart.webp").as_slice()
                }
                sow_core::player::Leader::Vercingetorix => {
                    include_bytes!("../../assets/avatars/vercingetorix.webp").as_slice()
                }
                sow_core::player::Leader::Boudica => {
                    include_bytes!("../../assets/avatars/boudica.webp").as_slice()
                }
                sow_core::player::Leader::LadySixSky => {
                    include_bytes!("../../assets/avatars/lady_six_sky.webp").as_slice()
                }
                sow_core::player::Leader::Leonidas => {
                    include_bytes!("../../assets/avatars/leonidas.webp").as_slice()
                }
                sow_core::player::Leader::Napoleon => {
                    include_bytes!("../../assets/avatars/napoleon.webp").as_slice()
                }
            };
            let name_lower = leader.name().to_lowercase().replace(' ', "_");
            self.avatars.insert(
                leader,
                load_image(&format!("avatar_{}", name_lower), avatar_bytes),
            );
        }

        self.avatar_fallback = Some(load_image(
            "avatar_null",
            include_bytes!("../../assets/avatars/null.webp"),
        ));
    }

    pub fn ui_splash_ready(&self) -> bool {
        self.ui_loader_empty.is_some()
            && self.ui_loader_full.is_some()
            && self.splash_desktop.is_some()
            && self.splash_mobile.is_some()
    }

    pub fn ingest_ui_splash_texture(
        &mut self,
        ctx: &egui::Context,
        kind: UiSplashTexture,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> bool {
        let expected = (width as usize).saturating_mul(height as usize).saturating_mul(4);
        if rgba.len() != expected {
            log::warn!(
                "ui splash texture {:?} size mismatch: got {} expected {}",
                kind,
                rgba.len(),
                expected
            );
            return false;
        }

        let color_image =
            egui::ColorImage::from_rgba_unmultiplied([width as _, height as _], rgba);
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

    pub fn ensure_ui_assets_loaded(&mut self, ctx: &egui::Context) {
        if !self.thumbnails.contains_key(sow_core::maps::DEFAULT_MAP_KEY) {
            let bytes = include_bytes!("../../../assets/maps/northamerica/thumbnail.webp");
            if let Some(color_image) =
                crate::ui::map_texture::color_image_from_map_thumbnail_bytes(bytes)
            {
                let key = sow_core::maps::DEFAULT_MAP_KEY.to_string();
                let texture =
                    ctx.load_texture(&key, color_image, egui::TextureOptions::LINEAR);
                self.thumbnails.insert(key, texture);
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            // Splash/bar textures are transferred from the HTML boot loader at WASM boot.
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.ui_loader_empty.is_some() {
                return;
            }

            let load_image = |name: &str, bytes: &[u8]| -> TextureHandle {
                let mut image = image::load_from_memory(bytes).expect("Failed to load UI asset");

                if image.width() > 2048 || image.height() > 2048 {
                    image = image.resize(2048, 2048, image::imageops::FilterType::Triangle);
                }

                let image_rgba = image.to_rgba8();
                let size = [image_rgba.width() as _, image_rgba.height() as _];
                let pixels = image_rgba.as_flat_samples();
                let color_image =
                    egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR)
            };

            self.ui_loader_empty = Some(load_image(
                "ui_loader_empty",
                include_bytes!("../../assets/ui/loader_empty.webp"),
            ));
            self.ui_loader_full = Some(load_image(
                "ui_loader_full",
                include_bytes!("../../assets/ui/loader_full.webp"),
            ));
            self.splash_desktop = Some(load_image(
                "sow_splash_desktop",
                include_bytes!("../../assets/ui/sow-splash-desktop.webp"),
            ));
            self.splash_mobile = Some(load_image(
                "sow_splash_mobile",
                include_bytes!("../../assets/ui/sow-splash-mobile.webp"),
            ));
        }
    }

    pub fn ensure_leaders_loaded(&mut self, ctx: &egui::Context) {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = ctx;
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        if !self.leader_desktop_images.is_empty() {
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        let load_image = |name: &str, bytes: &[u8]| -> TextureHandle {
            let mut image = image::load_from_memory(bytes).expect("Failed to load leader image");
            if image.width() > 2048 || image.height() > 2048 {
                image = image.resize(2048, 2048, image::imageops::FilterType::Triangle);
            }
            let image_rgba = image.to_rgba8();
            let size = [image_rgba.width() as _, image_rgba.height() as _];
            let pixels = image_rgba.as_flat_samples();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR)
        };

        #[cfg(not(target_arch = "wasm32"))]
        for &leader in &Leader::ALL {
            let desktop_bytes = match leader {
                sow_core::player::Leader::Caesar => {
                    include_bytes!("../../assets/ui/leaders/caesar_desktop.webp").as_slice()
                }
                sow_core::player::Leader::Cleopatra => {
                    include_bytes!("../../assets/ui/leaders/cleopatra_desktop.webp").as_slice()
                }
                sow_core::player::Leader::Ragnar => {
                    include_bytes!("../../assets/ui/leaders/ragnar_desktop.webp").as_slice()
                }
                sow_core::player::Leader::SunTzu => {
                    include_bytes!("../../assets/ui/leaders/sun_tzu_desktop.webp").as_slice()
                }
                sow_core::player::Leader::Alexander => {
                    include_bytes!("../../assets/ui/leaders/alexander_desktop.webp").as_slice()
                }
                sow_core::player::Leader::GenghisKhan => {
                    include_bytes!("../../assets/ui/leaders/genghis_khan_desktop.webp").as_slice()
                }
                sow_core::player::Leader::RichardTheLionheart => {
                    include_bytes!("../../assets/ui/leaders/richard_the_lionheart_desktop.webp")
                        .as_slice()
                }
                sow_core::player::Leader::Vercingetorix => {
                    include_bytes!("../../assets/ui/leaders/vercingetorix_desktop.webp").as_slice()
                }
                sow_core::player::Leader::Boudica => {
                    include_bytes!("../../assets/ui/leaders/boudica_desktop.webp").as_slice()
                }
                sow_core::player::Leader::LadySixSky => {
                    include_bytes!("../../assets/ui/leaders/lady_six_sky_desktop.webp").as_slice()
                }
                sow_core::player::Leader::Leonidas => {
                    include_bytes!("../../assets/ui/leaders/leonidas_desktop.webp").as_slice()
                }
                sow_core::player::Leader::Napoleon => {
                    include_bytes!("../../assets/ui/leaders/napoleon_desktop.webp").as_slice()
                }
            };
            let mobile_bytes = match leader {
                sow_core::player::Leader::Caesar => {
                    include_bytes!("../../assets/ui/leaders/caesar_mobile.webp").as_slice()
                }
                sow_core::player::Leader::Cleopatra => {
                    include_bytes!("../../assets/ui/leaders/cleopatra_mobile.webp").as_slice()
                }
                sow_core::player::Leader::Ragnar => {
                    include_bytes!("../../assets/ui/leaders/ragnar_mobile.webp").as_slice()
                }
                sow_core::player::Leader::SunTzu => {
                    include_bytes!("../../assets/ui/leaders/sun_tzu_mobile.webp").as_slice()
                }
                sow_core::player::Leader::Alexander => {
                    include_bytes!("../../assets/ui/leaders/alexander_mobile.webp").as_slice()
                }
                sow_core::player::Leader::GenghisKhan => {
                    include_bytes!("../../assets/ui/leaders/genghis_khan_mobile.webp").as_slice()
                }
                sow_core::player::Leader::RichardTheLionheart => {
                    include_bytes!("../../assets/ui/leaders/richard_the_lionheart_mobile.webp")
                        .as_slice()
                }
                sow_core::player::Leader::Vercingetorix => {
                    include_bytes!("../../assets/ui/leaders/vercingetorix_mobile.webp").as_slice()
                }
                sow_core::player::Leader::Boudica => {
                    include_bytes!("../../assets/ui/leaders/boudica_mobile.webp").as_slice()
                }
                sow_core::player::Leader::LadySixSky => {
                    include_bytes!("../../assets/ui/leaders/lady_six_sky_mobile.webp").as_slice()
                }
                sow_core::player::Leader::Leonidas => {
                    include_bytes!("../../assets/ui/leaders/leonidas_mobile.webp").as_slice()
                }
                sow_core::player::Leader::Napoleon => {
                    include_bytes!("../../assets/ui/leaders/napoleon_mobile.webp").as_slice()
                }
            };
            let name_lower = leader.name().to_lowercase().replace(' ', "_");
            self.leader_desktop_images.insert(
                leader,
                load_image(&format!("leader_{}_desktop", name_lower), desktop_bytes),
            );
            self.leader_mobile_images.insert(
                leader,
                load_image(&format!("leader_{}_mobile", name_lower), mobile_bytes),
            );
        }
    }

    pub fn ensure_hud_icons_loaded(&mut self, ctx: &egui::Context) {
        use crate::ui::hud::icons::HudIcon;

        if self.hud_icons.len() == HudIcon::ALL.len() {
            return;
        }

        for icon in HudIcon::ALL {
            if self.hud_icons.contains_key(&icon) {
                continue;
            }
            let image = image::load_from_memory(icon.bytes())
                .unwrap_or_else(|e| panic!("Failed to load {}: {e}", icon.file_name()))
                .to_rgba8();
            let size = [image.width() as _, image.height() as _];
            let pixels = image.as_flat_samples();
            let color_image =
                egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            let texture = ctx.load_texture(icon.texture_name(), color_image, egui::TextureOptions::LINEAR);
            self.hud_icons.insert(icon, texture);
        }
    }

    #[inline]
    pub fn hud_icon(&self, icon: crate::ui::hud::icons::HudIcon) -> Option<&TextureHandle> {
        self.hud_icons.get(&icon)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_bundled_map_br_present() {
        assert!(sow_core::maps::bundled_map_br("northamerica").is_some());
        assert!(sow_core::maps::bundled_map_br("world").is_none());
    }

    #[test]
    fn test_thumbnail_decoding() {
        let bytes = include_bytes!("../../../assets/maps/northamerica/thumbnail.webp");
        assert!(image::load_from_memory(bytes).is_ok());
    }
}
