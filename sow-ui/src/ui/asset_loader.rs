use egui::TextureHandle;
use std::collections::{HashMap, HashSet};

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
    pub avatars: Vec<TextureHandle>,
    pub avatar_fallback: Option<TextureHandle>,
    pub ui_loader_empty: Option<TextureHandle>,
    pub ui_loader_full: Option<TextureHandle>,
    pub splash_desktop: Option<TextureHandle>,
    pub splash_mobile: Option<TextureHandle>,
}

impl Default for AssetLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetLoader {
    pub fn new() -> Self {
        let mut manifests = HashMap::new();
        let mut map_catalog = None;

        let mut catalog = Vec::new();

        if let Ok(manifest) = serde_json::from_slice::<sow_core::map_legacy::MapManifest>(include_bytes!("../../../assets/maps/world/manifest.json")) {
            manifests.insert("world".to_string(), manifest.clone());
            catalog.push(manifest.clone());

            let mut custom_manifest = manifest.clone();
            custom_manifest.name = "Custom".to_string();
            custom_manifest.map.width = 800;
            custom_manifest.map.height = 600;
            manifests.insert("custom".to_string(), custom_manifest);
        }

        if let Ok(manifest) = serde_json::from_slice::<sow_core::map_legacy::MapManifest>(include_bytes!("../../../assets/maps/giantworldmap/manifest.json")) {
            manifests.insert("giantworldmap".to_string(), manifest.clone());
            catalog.push(manifest);
        }

        if let Ok(manifest) = serde_json::from_slice::<sow_core::map_legacy::MapManifest>(include_bytes!("../../../assets/maps/tutorial/manifest.json")) {
            manifests.insert("tutorial".to_string(), manifest.clone());
            catalog.push(manifest);
        }

        if !catalog.is_empty() {
            map_catalog = Some(catalog);
        }

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
            avatars: Vec::new(),
            avatar_fallback: None,
            ui_loader_empty: None,
            ui_loader_full: None,
            splash_desktop: None,
            splash_mobile: None,
        }
    }

    pub fn has_map(&self, map_name: &str) -> bool {
        map_name == "world" || map_name == "giantworldmap" || map_name == "tutorial" || self.maps.contains_key(map_name)
    }

    pub fn take_map(&mut self, map_name: &str) -> Option<Vec<u8>> {
        if map_name == "world" && !self.maps.contains_key("world") {
            self.maps.insert("world".to_string(), include_bytes!("../../../assets/maps/world/map.bin.br").to_vec());
        }
        if map_name == "giantworldmap" && !self.maps.contains_key("giantworldmap") {
            self.maps.insert("giantworldmap".to_string(), include_bytes!("../../../assets/maps/giantworldmap/map.bin.br").to_vec());
        }
        if map_name == "tutorial" {
            Some(include_bytes!("../../../assets/maps/tutorial/map.bin.br").to_vec())
        } else {
            self.maps.remove(map_name)
        }
    }

    pub fn thumbnail(&self, map_name: &str) -> Option<&TextureHandle> {
        self.thumbnails.get(map_name)
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
            let image = image::load_from_memory(bytes).expect("Failed to load avatar").to_rgba8();
            let size = [image.width() as _, image.height() as _];
            let pixels = image.as_flat_samples();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR)
        };

        self.avatars.push(load_image("avatar_0", include_bytes!("../../assets/avatars/0.webp")));
        self.avatars.push(load_image("avatar_1", include_bytes!("../../assets/avatars/1.webp")));
        self.avatars.push(load_image("avatar_2", include_bytes!("../../assets/avatars/2.webp")));
        self.avatars.push(load_image("avatar_3", include_bytes!("../../assets/avatars/3.webp")));
        self.avatars.push(load_image("avatar_4", include_bytes!("../../assets/avatars/4.webp")));
        self.avatars.push(load_image("avatar_5", include_bytes!("../../assets/avatars/5.webp")));
        self.avatars.push(load_image("avatar_6", include_bytes!("../../assets/avatars/6.webp")));
        self.avatars.push(load_image("avatar_7", include_bytes!("../../assets/avatars/7.webp")));

        self.avatar_fallback = Some(load_image("avatar_null", include_bytes!("../../assets/avatars/null.webp")));
    }

    pub fn ensure_ui_assets_loaded(&mut self, ctx: &egui::Context) {
        if self.ui_loader_empty.is_some() {
            return;
        }

        let load_image = |name: &str, bytes: &[u8]| -> TextureHandle {
            let mut image = image::load_from_memory(bytes).expect("Failed to load UI asset");
            
            // eGui has a maximum texture side limit (often 2048). Scale it down if it's too large.
            if image.width() > 2048 || image.height() > 2048 {
                image = image.resize(2048, 2048, image::imageops::FilterType::Triangle);
            }
            
            let image_rgba = image.to_rgba8();
            let size = [image_rgba.width() as _, image_rgba.height() as _];
            let pixels = image_rgba.as_flat_samples();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR)
        };

        self.ui_loader_empty = Some(load_image("ui_loader_empty", include_bytes!("../../assets/ui/loader_empty.webp")));
        self.ui_loader_full = Some(load_image("ui_loader_full", include_bytes!("../../assets/ui/loader_full.webp")));
        self.splash_desktop = Some(load_image("sow_splash_desktop", include_bytes!("../../assets/ui/sow-splash-desktop.webp")));
        self.splash_mobile = Some(load_image("sow_splash_mobile", include_bytes!("../../assets/ui/sow-splash-mobile.webp")));

        // Load the embedded world map thumbnail
        if !self.thumbnails.contains_key("world") {
            let bytes = include_bytes!("../../../assets/maps/world/thumbnail.webp");
            if let Ok(img) = image::load_from_memory(bytes) {
                let size = [img.width() as _, img.height() as _];
                let image_buffer = img.to_rgba8();
                let pixels = image_buffer.as_flat_samples();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                let texture = ctx.load_texture(
                    "world",
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                self.thumbnails.insert("world".to_string(), texture);
            }
        }

        // Load the embedded giantworldmap thumbnail
        if !self.thumbnails.contains_key("giantworldmap") {
            let bytes = include_bytes!("../../../assets/maps/giantworldmap/thumbnail.webp");
            if let Ok(img) = image::load_from_memory(bytes) {
                let size = [img.width() as _, img.height() as _];
                let image_buffer = img.to_rgba8();
                let pixels = image_buffer.as_flat_samples();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                let texture = ctx.load_texture(
                    "giantworldmap",
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                self.thumbnails.insert("giantworldmap".to_string(), texture);
            }
        }

        // Load the embedded tutorial map thumbnail
        if !self.thumbnails.contains_key("tutorial") {
            let bytes = include_bytes!("../../../assets/maps/tutorial/thumbnail.webp");
            if let Ok(img) = image::load_from_memory(bytes) {
                let size = [img.width() as _, img.height() as _];
                let image_buffer = img.to_rgba8();
                let pixels = image_buffer.as_flat_samples();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                let texture = ctx.load_texture(
                    "tutorial",
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                self.thumbnails.insert("tutorial".to_string(), texture);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_manifest_parsing() {
        let manifest_bytes = include_bytes!("../../../assets/maps/world/manifest.json");
        let parsed = serde_json::from_slice::<sow_core::map_legacy::MapManifest>(manifest_bytes);
        assert!(parsed.is_ok(), "Failed to parse manifest: {:?}", parsed.err());
        
        let loader = AssetLoader::new();
        assert!(loader.manifests.contains_key("world"));
        assert!(loader.manifests.contains_key("giantworldmap"));
        assert!(loader.manifests.contains_key("tutorial"));
        assert!(loader.manifests.contains_key("custom"));
    }
}