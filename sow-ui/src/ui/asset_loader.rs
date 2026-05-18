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
    /// Expected MD5 hashes for maps
    pub expected_md5s: HashMap<String, String>,
    /// Pre-loaded avatar textures
    pub avatars: Vec<TextureHandle>,
    pub avatar_fallback: Option<TextureHandle>,
}

impl Default for AssetLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetLoader {
    pub fn new() -> Self {
        Self {
            maps: HashMap::new(),
            maps_in_flight: HashSet::new(),
            thumbnails: HashMap::new(),
            thumbnails_in_flight: HashSet::new(),
            expected_md5s: HashMap::new(),
            avatars: Vec::new(),
            avatar_fallback: None,
        }
    }

    pub fn has_map(&self, map_name: &str) -> bool {
        self.maps.contains_key(map_name)
    }

    pub fn take_map(&mut self, map_name: &str) -> Option<Vec<u8>> {
        self.maps.remove(map_name)
    }

    pub fn thumbnail(&self, map_name: &str) -> Option<&TextureHandle> {
        self.thumbnails.get(map_name)
    }

    pub fn get_assets_to_fetch(
        &mut self,
        lobbies: &[sow_core::protocol::LobbyInfo],
    ) -> (Vec<String>, Vec<String>) {
        let mut thumbs_to_fetch = Vec::new();
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
}
