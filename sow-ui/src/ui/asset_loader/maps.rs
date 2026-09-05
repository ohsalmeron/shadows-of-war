use super::*;

impl AssetLoader {
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

    /// Queue thumbnails for every map eligible for matchmaking before the
    /// rotating lobby can select one. Native egui owns these previews; the
    /// browser menu has its own asset pipeline.
    pub fn prefetch_matchmaking_thumbnails(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.matchmaking_thumbnails_prefetched {
                return;
            }
            let Some(catalog) = self.map_catalog.as_ref() else {
                return;
            };
            let keys: Vec<String> = catalog
                .iter()
                .filter(|entry| entry.multiplayer_frequency > 0)
                .map(|entry| entry.key.clone())
                .collect();
            for key in keys {
                self.request_thumbnail(&key);
            }
            self.matchmaking_thumbnails_prefetched = true;
        }
    }

    pub fn drain_thumbnail_fetch_pending(&mut self) -> Vec<String> {
        self.thumbnails_fetch_pending.drain(..).collect()
    }
}
