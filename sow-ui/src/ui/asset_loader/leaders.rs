use super::*;

impl AssetLoader {
    pub(crate) fn leader_portrait_loaded(&self, key: LeaderPortraitKey) -> bool {
        if key.mobile {
            self.leader_mobile_images.contains_key(&key.leader)
        } else {
            self.leader_desktop_images.contains_key(&key.leader)
        }
    }

    /// Queue a leader portrait download on wasm32 (no-op when already loaded or in flight).
    pub fn request_leader_portrait(&mut self, leader: Leader, mobile: bool) {
        self.queue_leader_portrait_fetch(leader, mobile, false);
    }

    /// Same as [`request_leader_portrait`] but jumps ahead of other pending fetches.
    pub fn request_leader_portrait_priority(&mut self, leader: Leader, mobile: bool) {
        self.queue_leader_portrait_fetch(leader, mobile, true);
    }

    pub(crate) fn queue_leader_portrait_fetch(
        &mut self,
        leader: Leader,
        mobile: bool,
        front: bool,
    ) {
        let key = LeaderPortraitKey { leader, mobile };
        if self.leader_portrait_loaded(key) || self.leaders_in_flight.contains(&key) {
            return;
        }
        if !self.leader_retry_ready(key, Instant::now()) {
            return;
        }
        if self.leaders_fetch_pending.contains(&key) {
            if front {
                self.leaders_fetch_pending.retain(|pending| *pending != key);
                self.leaders_fetch_pending.insert(0, key);
            }
            return;
        }
        if front {
            self.leaders_fetch_pending.insert(0, key);
        } else {
            self.leaders_fetch_pending.push(key);
        }
    }

    /// Fetch/decode only this leader+layout; cancels queued work for other heroes.
    pub fn set_leader_portrait_focus(&mut self, leader: Leader, mobile: bool) {
        let key = LeaderPortraitKey { leader, mobile };
        if self.leader_portrait_focus == Some(key) {
            return;
        }
        self.leader_portrait_focus = Some(key);
        self.leaders_fetch_pending.retain(|pending| *pending == key);
        self.leader_decode_pending
            .retain(|(pending, _)| *pending == key);
        if !self.leader_portrait_loaded(key) {
            self.request_leader_portrait_priority(leader, mobile);
        }
    }

    pub fn leader_portrait_texture(&self, leader: Leader, mobile: bool) -> Option<&TextureHandle> {
        if mobile {
            self.leader_mobile_images.get(&leader)
        } else {
            self.leader_desktop_images.get(&leader)
        }
    }

    pub fn leader_portrait_ready(&self, leader: Leader, mobile: bool) -> bool {
        self.leader_portrait_loaded(LeaderPortraitKey { leader, mobile })
    }

    /// Pop the next portrait key to fetch (priority first), marking it in-flight.
    pub fn take_next_leader_fetch_pending(
        &mut self,
        priority: LeaderPortraitKey,
    ) -> Option<LeaderPortraitKey> {
        let now = Instant::now();
        let mut checked = 0usize;
        loop {
            if self.leaders_in_flight.len() >= MAX_LEADER_FETCHES_IN_FLIGHT {
                return None;
            }
            if checked >= self.leaders_fetch_pending.len() {
                return None;
            }

            let key = if let Some(i) = self
                .leaders_fetch_pending
                .iter()
                .position(|pending| *pending == priority)
            {
                self.leaders_fetch_pending.remove(i)
            } else if !self.leaders_fetch_pending.is_empty() {
                self.leaders_fetch_pending.remove(0)
            } else {
                return None;
            };

            if self.leader_portrait_loaded(key) {
                continue;
            }
            if !self.leader_retry_ready(key, now) {
                self.leaders_fetch_pending.push(key);
                checked += 1;
                continue;
            }

            self.leaders_in_flight.insert(key);
            return Some(key);
        }
    }

    pub fn note_leader_portrait_fetch_failed(
        &mut self,
        leader: Leader,
        mobile: bool,
        reason: impl Into<String>,
    ) {
        let key = LeaderPortraitKey { leader, mobile };
        self.leaders_in_flight.remove(&key);
        let now = Instant::now();
        let reason = reason.into();
        let permanent = reason.contains("404");
        let attempts = self
            .leader_retry_state
            .get(&key)
            .map(|state| state.attempts.saturating_add(1))
            .unwrap_or(1);
        let exp = attempts.saturating_sub(1).min(4);
        let delay_ms = 500u64.saturating_mul(1u64 << exp);
        self.leader_retry_state.insert(
            key,
            PortraitRetryState {
                attempts,
                next_retry_at: now + Duration::from_millis(delay_ms),
                last_error: reason,
                permanent,
            },
        );

        if self.leader_portrait_focus == Some(key)
            && !permanent
            && !self.leaders_fetch_pending.contains(&key)
        {
            self.leaders_fetch_pending.push(key);
        }
    }

    pub fn leader_retry_debug(
        &self,
        leader: Leader,
        mobile: bool,
    ) -> Option<(u32, Duration, &str)> {
        let key = LeaderPortraitKey { leader, mobile };
        let state = self.leader_retry_state.get(&key)?;
        let now = Instant::now();
        let remaining = if state.next_retry_at > now {
            state.next_retry_at - now
        } else {
            Duration::from_millis(0)
        };
        Some((state.attempts, remaining, state.last_error.as_str()))
    }

    pub fn leader_portrait_filename(key: LeaderPortraitKey) -> String {
        let name_lower = Self::leader_slug(key.leader);
        let form = if key.mobile { "mobile" } else { "desktop" };
        format!("{name_lower}_{form}.webp")
    }

    pub(crate) fn decode_leader_portrait_bytes(bytes: &[u8]) -> Result<egui::ColorImage, String> {
        let mut image = image::load_from_memory(bytes).map_err(|e| format!("decode: {e}"))?;
        if image.width() > 2048 || image.height() > 2048 {
            image = image.resize(2048, 2048, image::imageops::FilterType::Triangle);
        }
        let image_rgba = image.to_rgba8();
        let size = [image_rgba.width() as _, image_rgba.height() as _];
        let pixels = image_rgba.as_flat_samples();
        Ok(egui::ColorImage::from_rgba_unmultiplied(
            size,
            pixels.as_slice(),
        ))
    }

    pub fn enqueue_leader_portrait_bytes(&mut self, leader: Leader, mobile: bool, bytes: Vec<u8>) {
        let key = LeaderPortraitKey { leader, mobile };
        self.leaders_in_flight.remove(&key);
        self.leader_retry_state.remove(&key);
        if self.leader_portrait_focus != Some(key) {
            log::warn!(
                "Leader portrait bytes discarded: received {:?} mobile={} but focus={:?}",
                leader,
                mobile,
                self.leader_portrait_focus
            );
            return;
        }
        self.leader_decode_pending.push_back((key, bytes));
    }

    pub(crate) fn drop_stale_leader_decodes(&mut self) {
        let Some(focus) = self.leader_portrait_focus else {
            self.leader_decode_pending.clear();
            return;
        };
        self.leader_decode_pending.retain(|(key, _)| *key == focus);
    }

    /// Decode and upload at most `max_per_frame` queued portraits for the focused leader.
    pub fn process_leader_decode_budget(
        &mut self,
        ctx: &egui::Context,
        max_per_frame: usize,
        focus: LeaderPortraitKey,
    ) {
        self.drop_stale_leader_decodes();
        for _ in 0..max_per_frame {
            let idx = self
                .leader_decode_pending
                .iter()
                .position(|(key, _)| *key == focus);
            let Some(i) = idx else { break };
            let (key, bytes) = self.leader_decode_pending.remove(i).unwrap();
            let leader = key.leader;
            let mobile = key.mobile;

            let color_image = match Self::decode_leader_portrait_bytes(&bytes) {
                Ok(img) => img,
                Err(e) => {
                    log::warn!(
                        "Failed to decode leader portrait {:?} mobile={}: {}",
                        leader,
                        mobile,
                        e
                    );
                    continue;
                }
            };

            let name_lower = Self::leader_slug(leader);
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
            self.leader_retry_state.remove(&key);
        }
    }
}
