use super::*;

impl AssetLoader {
    pub(crate) fn avatar_loaded(&self, key: AvatarFetchKey) -> bool {
        match key {
            AvatarFetchKey::Fallback => self.avatar_fallback.is_some(),
            AvatarFetchKey::Leader(leader) => self.avatars.contains_key(&leader),
        }
    }

    pub(crate) fn avatar_retry_ready(&self, key: AvatarFetchKey, now: Instant) -> bool {
        match self.avatar_retry_state.get(&key) {
            Some(state) if state.permanent => false,
            Some(state) => now >= state.next_retry_at,
            None => true,
        }
    }

    pub fn request_avatars_fetch_all(&mut self) {
        if self.avatars_fetch_all_queued {
            return;
        }
        self.avatars_fetch_all_queued = true;
        self.queue_avatar_fetch(AvatarFetchKey::Fallback, true);
        for &leader in &Leader::ALL {
            self.queue_avatar_fetch(AvatarFetchKey::Leader(leader), false);
        }
    }

    pub fn request_avatar_priority(&mut self, leader: Leader) {
        self.queue_avatar_fetch(AvatarFetchKey::Leader(leader), true);
    }

    pub(crate) fn queue_avatar_fetch(&mut self, key: AvatarFetchKey, front: bool) {
        if self.avatar_loaded(key) || self.avatars_in_flight.contains(&key) {
            return;
        }
        if !self.avatar_retry_ready(key, Instant::now()) {
            return;
        }
        if self.avatars_fetch_pending.contains(&key) {
            if front {
                self.avatars_fetch_pending.retain(|pending| *pending != key);
                self.avatars_fetch_pending.insert(0, key);
            }
            return;
        }
        if front {
            self.avatars_fetch_pending.insert(0, key);
        } else {
            self.avatars_fetch_pending.push(key);
        }
    }

    pub fn take_next_avatar_fetch_pending(
        &mut self,
        priority: AvatarFetchKey,
    ) -> Option<AvatarFetchKey> {
        let now = Instant::now();
        let mut checked = 0usize;
        loop {
            if self.avatars_in_flight.len() >= MAX_AVATAR_FETCHES_IN_FLIGHT {
                return None;
            }
            if checked >= self.avatars_fetch_pending.len() {
                return None;
            }

            let key = if let Some(i) = self
                .avatars_fetch_pending
                .iter()
                .position(|pending| *pending == priority)
            {
                self.avatars_fetch_pending.remove(i)
            } else if !self.avatars_fetch_pending.is_empty() {
                self.avatars_fetch_pending.remove(0)
            } else {
                return None;
            };

            if self.avatar_loaded(key) {
                continue;
            }
            if !self.avatar_retry_ready(key, now) {
                self.avatars_fetch_pending.push(key);
                checked += 1;
                continue;
            }

            self.avatars_in_flight.insert(key);
            return Some(key);
        }
    }

    pub fn note_avatar_fetch_failed(&mut self, key: AvatarFetchKey, reason: impl Into<String>) {
        self.avatars_in_flight.remove(&key);
        let now = Instant::now();
        let reason = reason.into();
        let permanent = reason.contains("404");
        let attempts = self
            .avatar_retry_state
            .get(&key)
            .map(|state| state.attempts.saturating_add(1))
            .unwrap_or(1);
        let exp = attempts.saturating_sub(1).min(4);
        let delay_ms = 500u64.saturating_mul(1u64 << exp);
        self.avatar_retry_state.insert(
            key,
            PortraitRetryState {
                attempts,
                next_retry_at: now + Duration::from_millis(delay_ms),
                last_error: reason,
                permanent,
            },
        );
        if !permanent && !self.avatars_fetch_pending.contains(&key) {
            self.avatars_fetch_pending.push(key);
        }
    }

    pub fn queue_portal_avatar(&mut self, url: String) {
        if self.portal_avatar.is_some() || self.portal_avatar_in_flight {
            return;
        }
        if self.portal_avatar_request.as_deref() == Some(url.as_str()) {
            return;
        }
        self.portal_avatar_request = Some(url);
    }

    pub fn note_portal_avatar_failed(&mut self, reason: impl Into<String>) {
        log::warn!("Portal avatar fetch failed: {}", reason.into());
        self.portal_avatar_in_flight = false;
        self.portal_avatar_request = None;
    }

    pub fn ingest_portal_avatar_bytes(
        &mut self,
        ctx: &egui::Context,
        bytes: &[u8],
    ) -> Result<(), String> {
        self.portal_avatar_in_flight = false;
        self.portal_avatar_request = None;

        let image = image::load_from_memory(bytes).map_err(|e| format!("decode avatar: {e}"))?;
        let image_rgba = image.to_rgba8();
        let size = [image_rgba.width() as _, image_rgba.height() as _];
        let pixels = image_rgba.as_flat_samples();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
        let texture = ctx.load_texture("portal_avatar", color_image, egui::TextureOptions::LINEAR);
        self.portal_avatar = Some(texture);
        Ok(())
    }

    pub fn ingest_avatar_webp_bytes(
        &mut self,
        ctx: &egui::Context,
        key: AvatarFetchKey,
        bytes: &[u8],
    ) -> Result<(), String> {
        self.avatars_in_flight.remove(&key);
        self.avatar_retry_state.remove(&key);

        let image = image::load_from_memory(bytes).map_err(|e| format!("decode avatar: {e}"))?;
        let image_rgba = image.to_rgba8();
        let size = [image_rgba.width() as _, image_rgba.height() as _];

        // Also stage a cell-sized RGBA copy for the GPU image atlas (world nameplates render
        // avatars through the text-emoji pipeline). `GPU_AVATAR_CELL` must match sow-render's
        // `AVATAR_CELL`. This is the one place all avatars (native file-load + web fetch) pass
        // through, so the GPU side is fed on every platform.
        const GPU_AVATAR_CELL: u32 = 128;
        let cell = image::imageops::resize(
            &image_rgba,
            GPU_AVATAR_CELL,
            GPU_AVATAR_CELL,
            image::imageops::FilterType::Triangle,
        );
        self.gpu_avatar_cells.retain(|(k, _)| *k != key);
        self.gpu_avatar_cells.push((key, cell.into_raw()));

        let pixels = image_rgba.as_flat_samples();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());

        match key {
            AvatarFetchKey::Fallback => {
                let texture =
                    ctx.load_texture("avatar_null", color_image, egui::TextureOptions::LINEAR);
                self.avatar_fallback = Some(texture);
            }
            AvatarFetchKey::Leader(leader) => {
                let tex_name = format!("avatar_{}", Self::leader_slug(leader));
                let texture =
                    ctx.load_texture(&tex_name, color_image, egui::TextureOptions::LINEAR);
                self.avatars.insert(leader, texture);
            }
        }
        Ok(())
    }
}
