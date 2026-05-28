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
    /// Fully downloaded + cached map binary data (compressed .br bytes)
    pub maps: HashMap<String, Vec<u8>>,
    /// Maps currently being fetched
    pub maps_in_flight: HashSet<String>,
    /// Decoded thumbnail textures ready for egui
    pub thumbnails: HashMap<String, TextureHandle>,
    /// Thumbnails currently being fetched
    pub thumbnails_in_flight: HashSet<String>,
    /// Fetched MapManifests
    pub manifests: HashMap<String, sow_core::map_legacy::MapManifest>,
    /// Manifests currently being fetched
    pub manifests_in_flight: HashSet<String>,
    pub catalog_in_flight: bool,
    /// The global list of all available maps fetched from maps.json
    pub map_catalog: Option<Vec<sow_core::map_legacy::MapManifest>>,
    /// Expected MD5 hashes for maps
    pub expected_md5s: HashMap<String, String>,
    /// Pre-loaded avatar textures
    pub avatars: HashMap<sow_core::player::Leader, TextureHandle>,
    pub avatar_fallback: Option<TextureHandle>,
    pub ui_loader_empty: Option<TextureHandle>,
    pub ui_loader_full: Option<TextureHandle>,
    pub leader_desktop_images: HashMap<sow_core::player::Leader, TextureHandle>,
    pub leader_mobile_images: HashMap<sow_core::player::Leader, TextureHandle>,
    pub splash_desktop: Option<TextureHandle>,
    pub splash_mobile: Option<TextureHandle>,
    /// Queued leader portrait fetches (wasm32); drained by sow-client network layer.
    pub leaders_fetch_pending: Vec<LeaderPortraitKey>,
    pub leaders_in_flight: HashSet<LeaderPortraitKey>,
}

impl Default for AssetLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetLoader {
    pub fn new() -> Self {
        let mut manifests = HashMap::new();
        let mut catalog = Vec::new();

        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Ok(manifest) = serde_json::from_slice::<sow_core::map_legacy::MapManifest>(
                include_bytes!("../../../assets/maps/world/manifest.json"),
            ) {
                manifests.insert("world".to_string(), manifest.clone());
                catalog.push(manifest.clone());

                let mut custom_manifest = manifest.clone();
                custom_manifest.name = "Custom".to_string();
                custom_manifest.map.width = 800;
                custom_manifest.map.height = 600;
                manifests.insert("custom".to_string(), custom_manifest);
            }

            if let Ok(manifest) = serde_json::from_slice::<sow_core::map_legacy::MapManifest>(
                include_bytes!("../../../assets/maps/giantworldmap/manifest.json"),
            ) {
                manifests.insert("giantworldmap".to_string(), manifest.clone());
                catalog.push(manifest);
            }
        }

        if let Ok(manifest) = serde_json::from_slice::<sow_core::map_legacy::MapManifest>(
            include_bytes!("../../../assets/maps/tutorial/manifest.json"),
        ) {
            manifests.insert("tutorial".to_string(), manifest.clone());
            catalog.push(manifest);
        }

        #[cfg(not(target_arch = "wasm32"))]
        let map_catalog = if catalog.is_empty() {
            None
        } else {
            Some(catalog)
        };
        #[cfg(target_arch = "wasm32")]
        let map_catalog: Option<Vec<sow_core::map_legacy::MapManifest>> = None;

        Self {
            maps: HashMap::new(),
            maps_in_flight: HashSet::new(),
            thumbnails: HashMap::new(),
            thumbnails_in_flight: HashSet::new(),
            manifests,
            manifests_in_flight: HashSet::new(),
            catalog_in_flight: false,
            map_catalog,
            expected_md5s: HashMap::new(),
            avatars: HashMap::new(),
            avatar_fallback: None,
            ui_loader_empty: None,
            ui_loader_full: None,
            leader_desktop_images: HashMap::new(),
            leader_mobile_images: HashMap::new(),
            splash_desktop: None,
            splash_mobile: None,
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
        let key = map_name.to_lowercase().replace([' ', '_'], "");
        key == "world"
            || key == "giantworldmap"
            || key == "tutorial"
            || self.maps.contains_key(&key)
    }

    pub fn take_map(&mut self, map_name: &str) -> Option<Vec<u8>> {
        let key = map_name.to_lowercase().replace([' ', '_'], "");
        #[cfg(not(target_arch = "wasm32"))]
        {
            if key == "world" && !self.maps.contains_key("world") {
                self.maps.insert(
                    "world".to_string(),
                    include_bytes!("../../../assets/maps/world/map.bin.br").to_vec(),
                );
            }
            if key == "giantworldmap" && !self.maps.contains_key("giantworldmap") {
                self.maps.insert(
                    "giantworldmap".to_string(),
                    include_bytes!("../../../assets/maps/giantworldmap/map.bin.br").to_vec(),
                );
            }
        }
        if key == "tutorial" {
            Some(include_bytes!("../../../assets/maps/tutorial/map.bin.br").to_vec())
        } else {
            self.maps.remove(&key)
        }
    }

    pub fn thumbnail(&self, map_name: &str) -> Option<&TextureHandle> {
        let key = map_name.to_lowercase().replace([' ', '_'], "");
        self.thumbnails.get(&key)
    }

    pub fn get_assets_to_fetch(
        &mut self,
        lobbies: &[sow_core::protocol::LobbyInfo],
    ) -> (Vec<String>, Vec<String>, Vec<String>) {
        let mut thumbs_to_fetch = Vec::new();
        let mut manifests_to_fetch = Vec::new();
        let mut maps_to_fetch = Vec::new();

        let mut unique_maps = HashSet::new();
        for l in lobbies {
            unique_maps.insert(l.map_name.clone());
            if let Some(md5) = &l.map_md5 {
                self.expected_md5s.insert(l.map_name.clone(), md5.clone());
            }
        }

        // Thumbnails: fetch all missing ones at once (they are small)
        for map_name in &unique_maps {
            if !self.thumbnails.contains_key(map_name)
                && !self.thumbnails_in_flight.contains(map_name)
            {
                self.thumbnails_in_flight.insert(map_name.clone());
                thumbs_to_fetch.push(map_name.clone());
            }
            if !self.manifests.contains_key(map_name)
                && !self.manifests_in_flight.contains(map_name)
            {
                self.manifests_in_flight.insert(map_name.clone());
                manifests_to_fetch.push(map_name.clone());
            }
        }

        // Maps: only fetch if no other map is currently downloading to prevent network congestion
        if self.maps_in_flight.is_empty() {
            // Pick primary map first, then arbitrary next
            let mut target_map = None;
            if let Some(primary) = crate::ui::main_menu::primary_lobby_for_browser(lobbies) {
                if !self.maps.contains_key(&primary.map_name) {
                    target_map = Some(primary.map_name.clone());
                }
            }
            if target_map.is_none() {
                for map_name in &unique_maps {
                    if !self.maps.contains_key(map_name) {
                        target_map = Some(map_name.clone());
                        break;
                    }
                }
            }

            if let Some(m) = target_map {
                self.maps_in_flight.insert(m.clone());
                maps_to_fetch.push(m);
            }
        }

        (thumbs_to_fetch, manifests_to_fetch, maps_to_fetch)
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

    pub fn ensure_ui_assets_loaded(&mut self, ctx: &egui::Context) {
        if !self.thumbnails.contains_key("tutorial") {
            let bytes = include_bytes!("../../../assets/maps/tutorial/thumbnail.webp");
            if let Ok(img) = image::load_from_memory(bytes) {
                let size = [img.width() as _, img.height() as _];
                let image_buffer = img.to_rgba8();
                let pixels = image_buffer.as_flat_samples();
                let color_image =
                    egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                let texture =
                    ctx.load_texture("tutorial", color_image, egui::TextureOptions::LINEAR);
                self.thumbnails.insert("tutorial".to_string(), texture);
            }
        }

        #[cfg(target_arch = "wasm32")]
        return;

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

            if !self.thumbnails.contains_key("world") {
                let bytes = include_bytes!("../../../assets/maps/world/thumbnail.webp");
                if let Ok(img) = image::load_from_memory(bytes) {
                    let size = [img.width() as _, img.height() as _];
                    let image_buffer = img.to_rgba8();
                    let pixels = image_buffer.as_flat_samples();
                    let color_image =
                        egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                    let texture =
                        ctx.load_texture("world", color_image, egui::TextureOptions::LINEAR);
                    self.thumbnails.insert("world".to_string(), texture);
                }
            }

            if !self.thumbnails.contains_key("giantworldmap") {
                let bytes = include_bytes!("../../../assets/maps/giantworldmap/thumbnail.webp");
                if let Ok(img) = image::load_from_memory(bytes) {
                    let size = [img.width() as _, img.height() as _];
                    let image_buffer = img.to_rgba8();
                    let pixels = image_buffer.as_flat_samples();
                    let color_image =
                        egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                    let texture = ctx.load_texture(
                        "giantworldmap",
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.thumbnails.insert("giantworldmap".to_string(), texture);
                }
            }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_manifest_parsing() {
        let loader = AssetLoader::new();
        assert!(loader.manifests.contains_key("tutorial"));
        #[cfg(not(target_arch = "wasm32"))]
        {
            let manifest_bytes = include_bytes!("../../../assets/maps/world/manifest.json");
            let parsed =
                serde_json::from_slice::<sow_core::map_legacy::MapManifest>(manifest_bytes);
            assert!(
                parsed.is_ok(),
                "Failed to parse manifest: {:?}",
                parsed.err()
            );
            assert!(loader.manifests.contains_key("world"));
            assert!(loader.manifests.contains_key("giantworldmap"));
            assert!(loader.manifests.contains_key("custom"));
        }
    }

    #[test]
    fn test_thumbnail_decoding() {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let world_bytes = include_bytes!("../../../assets/maps/world/thumbnail.webp");
            assert!(image::load_from_memory(world_bytes).is_ok());
            let giant_bytes = include_bytes!("../../../assets/maps/giantworldmap/thumbnail.webp");
            assert!(image::load_from_memory(giant_bytes).is_ok());
        }
        let tutorial_bytes = include_bytes!("../../../assets/maps/tutorial/thumbnail.webp");
        assert!(image::load_from_memory(tutorial_bytes).is_ok());
    }
}
