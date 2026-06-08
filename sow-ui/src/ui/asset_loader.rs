use egui::TextureHandle;
use sow_core::player::Leader;
use std::collections::{HashMap, HashSet, VecDeque};
use web_time::{Duration, Instant};

/// Max leader portrait HTTP requests at once (wasm) — only one hero is shown at a time.
pub const MAX_LEADER_FETCHES_IN_FLIGHT: usize = 1;

/// Boot splash/loader webp fetches in parallel during wasm cold start.
pub const MAX_BOOT_UI_FETCHES_IN_FLIGHT: usize = 4;

/// Leader rail / HUD avatar webp fetches (wasm32).
pub const MAX_AVATAR_FETCHES_IN_FLIGHT: usize = 6;

/// Queued avatar download (`Fallback` = `null.webp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AvatarFetchKey {
    Fallback,
    Leader(Leader),
}

/// Desktop vs mobile leader portrait variant (CDN on wasm32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LeaderPortraitKey {
    pub leader: Leader,
    pub mobile: bool,
}

#[derive(Debug, Clone)]
struct PortraitRetryState {
    attempts: u32,
    next_retry_at: Instant,
    last_error: String,
    permanent: bool,
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
    /// Boot loader/splash webp (wasm32 HTTP).
    pub boot_ui_fetch_pending: Vec<UiSplashTexture>,
    pub boot_ui_in_flight: HashSet<UiSplashTexture>,
    /// Raw portrait bytes awaiting decode (keeps main thread responsive on wasm).
    leader_decode_pending: VecDeque<(LeaderPortraitKey, Vec<u8>)>,
    /// Portrait currently shown / being loaded (drops stale fetch+decode work).
    leader_portrait_focus: Option<LeaderPortraitKey>,
    /// Retry policy state per failed leader portrait fetch.
    leader_retry_state: HashMap<LeaderPortraitKey, PortraitRetryState>,
    /// Last portrait we successfully uploaded; used as a non-blocking fallback.
    last_ready_portrait: Option<LeaderPortraitKey>,
    /// Avatar CDN fetches (wasm32).
    avatars_fetch_pending: Vec<AvatarFetchKey>,
    pub avatars_in_flight: HashSet<AvatarFetchKey>,
    avatars_fetch_all_queued: bool,
    avatar_retry_state: HashMap<AvatarFetchKey, PortraitRetryState>,
}

impl Default for AssetLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Splash / loader bar textures used by the loading screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiSplashTexture {
    LoaderEmpty,
    LoaderFull,
    SplashDesktop,
    SplashMobile,
}

#[cfg(test)]
mod avatar_tests {
    use super::{AvatarFetchKey, AssetLoader};
    use sow_core::player::Leader;

    #[test]
    fn avatar_filenames_match_static_tree() {
        assert_eq!(AssetLoader::avatar_filename(AvatarFetchKey::Fallback), "null.webp");
        assert_eq!(
            AssetLoader::avatar_filename(AvatarFetchKey::Leader(Leader::SunTzu)),
            "sun_tzu.webp"
        );
    }
}

impl UiSplashTexture {
    pub const ALL: [Self; 4] = [
        Self::LoaderEmpty,
        Self::LoaderFull,
        Self::SplashDesktop,
        Self::SplashMobile,
    ];

    pub fn filename(self) -> &'static str {
        match self {
            Self::LoaderEmpty => "loader_empty.webp",
            Self::LoaderFull => "loader_full.webp",
            Self::SplashDesktop => "sow-splash-desktop.webp",
            Self::SplashMobile => "sow-splash-mobile.webp",
        }
    }

    fn loaded_in(self, loader: &AssetLoader) -> bool {
        match self {
            Self::LoaderEmpty => loader.ui_loader_empty.is_some(),
            Self::LoaderFull => loader.ui_loader_full.is_some(),
            Self::SplashDesktop => loader.splash_desktop.is_some(),
            Self::SplashMobile => loader.splash_mobile.is_some(),
        }
    }
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
            boot_ui_fetch_pending: Vec::new(),
            boot_ui_in_flight: HashSet::new(),
            leader_decode_pending: VecDeque::new(),
            leader_portrait_focus: None,
            leader_retry_state: HashMap::new(),
            last_ready_portrait: None,
            avatars_fetch_pending: Vec::new(),
            avatars_in_flight: HashSet::new(),
            avatars_fetch_all_queued: false,
            avatar_retry_state: HashMap::new(),
        }
    }

    pub fn leader_slug(leader: Leader) -> String {
        leader.name().to_lowercase().replace(' ', "_")
    }

    pub fn avatar_filename(key: AvatarFetchKey) -> String {
        match key {
            AvatarFetchKey::Fallback => "null.webp".to_string(),
            AvatarFetchKey::Leader(leader) => format!("{}.webp", Self::leader_slug(leader)),
        }
    }

    fn avatar_loaded(&self, key: AvatarFetchKey) -> bool {
        match key {
            AvatarFetchKey::Fallback => self.avatar_fallback.is_some(),
            AvatarFetchKey::Leader(leader) => self.avatars.contains_key(&leader),
        }
    }

    fn avatar_retry_ready(&self, key: AvatarFetchKey, now: Instant) -> bool {
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

    fn queue_avatar_fetch(&mut self, key: AvatarFetchKey, front: bool) {
        if self.avatar_loaded(key) || self.avatars_in_flight.contains(&key) {
            return;
        }
        if !self.avatar_retry_ready(key, Instant::now()) {
            return;
        }
        if self.avatars_fetch_pending.iter().any(|pending| *pending == key) {
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
        if !permanent && !self.avatars_fetch_pending.iter().any(|pending| *pending == key) {
            self.avatars_fetch_pending.push(key);
        }
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
        let pixels = image_rgba.as_flat_samples();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());

        match key {
            AvatarFetchKey::Fallback => {
                let texture = ctx.load_texture(
                    "avatar_null",
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                self.avatar_fallback = Some(texture);
            }
            AvatarFetchKey::Leader(leader) => {
                let tex_name = format!("avatar_{}", Self::leader_slug(leader));
                let texture = ctx.load_texture(&tex_name, color_image, egui::TextureOptions::LINEAR);
                self.avatars.insert(leader, texture);
            }
        }
        Ok(())
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
        self.queue_leader_portrait_fetch(leader, mobile, false);
    }

    /// Same as [`request_leader_portrait`] but jumps ahead of other pending fetches.
    pub fn request_leader_portrait_priority(&mut self, leader: Leader, mobile: bool) {
        self.queue_leader_portrait_fetch(leader, mobile, true);
    }

    fn queue_leader_portrait_fetch(&mut self, leader: Leader, mobile: bool, front: bool) {
        let key = LeaderPortraitKey { leader, mobile };
        if self.leader_portrait_loaded(key) || self.leaders_in_flight.contains(&key) {
            return;
        }
        if !self.leader_retry_ready(key, Instant::now()) {
            return;
        }
        if self.leaders_fetch_pending.iter().any(|pending| *pending == key) {
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
        self.leader_decode_pending.retain(|(pending, _)| *pending == key);
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

    /// Last successfully uploaded portrait for this layout (no Caesar default).
    pub fn fallback_leader_portrait_texture(&self, mobile: bool) -> Option<&TextureHandle> {
        if let Some(last) = self.last_ready_portrait {
            if last.mobile == mobile {
                return self.leader_portrait_texture(last.leader, mobile);
            }
        }
        None
    }

    pub fn best_leader_portrait_texture(
        &self,
        leader: Leader,
        mobile: bool,
    ) -> Option<&TextureHandle> {
        self.leader_portrait_texture(leader, mobile)
            .or_else(|| self.fallback_leader_portrait_texture(mobile))
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
            && !self.leaders_fetch_pending.iter().any(|pending| *pending == key)
        {
            self.leaders_fetch_pending.push(key);
        }
    }

    pub fn leader_retry_debug(&self, leader: Leader, mobile: bool) -> Option<(u32, Duration, &str)> {
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
        let name_lower = key.leader.name().to_lowercase().replace(' ', "_");
        let form = if key.mobile { "mobile" } else { "desktop" };
        format!("{name_lower}_{form}.webp")
    }

    fn decode_leader_portrait_bytes(bytes: &[u8]) -> Result<egui::ColorImage, String> {
        let mut image = image::load_from_memory(bytes)
            .map_err(|e| format!("decode: {e}"))?;
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
            return;
        }
        self.leader_decode_pending.push_back((key, bytes));
    }

    fn drop_stale_leader_decodes(&mut self) {
        let Some(focus) = self.leader_portrait_focus else {
            self.leader_decode_pending.clear();
            return;
        };
        self.leader_decode_pending
            .retain(|(key, _)| *key == focus);
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
            self.last_ready_portrait = Some(key);
            self.leader_retry_state.remove(&key);
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
        #[cfg(target_arch = "wasm32")]
        {
            let _ = ctx;
            self.request_avatars_fetch_all();
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.avatar_fallback.is_some() && self.avatars.len() >= Leader::ALL.len() {
                return;
            }

            fn read_avatar_webp(filename: &str) -> Option<Vec<u8>> {
                use std::path::Path;
                let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../assets/cdn/avatars")
                    .join(filename);
                std::fs::read(&path).ok()
            }

            let load_key = |key: AvatarFetchKey| -> Option<(AvatarFetchKey, Vec<u8>)> {
                let filename = Self::avatar_filename(key);
                read_avatar_webp(&filename).map(|bytes| (key, bytes))
            };

            for key in std::iter::once(AvatarFetchKey::Fallback).chain(
                Leader::ALL
                    .iter()
                    .copied()
                    .map(AvatarFetchKey::Leader),
            ) {
                if self.avatar_loaded(key) {
                    continue;
                }
                let Some((key, bytes)) = load_key(key) else {
                    log::warn!(
                        "missing avatar {} in assets/cdn/avatars/",
                        Self::avatar_filename(key)
                    );
                    continue;
                };
                if let Err(e) = self.ingest_avatar_webp_bytes(ctx, key, &bytes) {
                    log::warn!("failed to load avatar {:?}: {e}", key);
                }
            }
        }
    }

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
        if self.boot_ui_fetch_pending.iter().any(|pending| *pending == kind) {
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

    pub fn boot_leader_ready(&self, leader: Leader, mobile: bool) -> bool {
        self.leader_portrait_ready(leader, mobile)
    }

    pub fn ensure_ui_assets_loaded(&mut self, ctx: &egui::Context) {
        if !self.thumbnails.contains_key(sow_core::maps::DEFAULT_MAP_KEY) {
            let bytes = sow_core::repo_asset_bytes!("maps/world/thumbnail.webp");
            if let Some(color_image) =
                crate::ui::map_texture::color_image_from_map_thumbnail_bytes(bytes)
            {
                let key = sow_core::maps::DEFAULT_MAP_KEY.to_string();
                let texture =
                    ctx.load_texture(&key, color_image, egui::TextureOptions::LINEAR);
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

        #[cfg(all(not(target_arch = "wasm32"), target_os = "ios"))]
        {
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
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../assets/cdn/ui/loader_empty.webp"
                )),
            ));
            self.ui_loader_full = Some(load_image(
                "ui_loader_full",
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../assets/cdn/ui/loader_full.webp"
                )),
            ));
            self.splash_desktop = Some(load_image(
                "sow_splash_desktop",
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../assets/cdn/ui/sow-splash-desktop.webp"
                )),
            ));
            self.splash_mobile = Some(load_image(
                "sow_splash_mobile",
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../assets/cdn/ui/sow-splash-mobile.webp"
                )),
            ));
        }

        #[cfg(all(not(target_arch = "wasm32"), not(target_os = "ios")))]
        {
            fn read_ui_webp(filename: &str) -> Vec<u8> {
                use std::path::Path;
                let base = Path::new(env!("CARGO_MANIFEST_DIR"));
                let static_p = base.join("../assets/static/ui").join(filename);
                let cdn_p = base.join("../assets/cdn/ui").join(filename);
                std::fs::read(&static_p)
                    .or_else(|_| std::fs::read(&cdn_p))
                    .unwrap_or_else(|e| {
                        panic!(
                            "missing UI asset {filename} in assets/static/ui or assets/cdn/ui: {e}"
                        )
                    })
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
            let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/cdn/leaders");
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
                    let name_lower = leader.name().to_lowercase().replace(' ', "_");
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
                    self.last_ready_portrait = Some(key);
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

impl AssetLoader {
    fn leader_retry_ready(&self, key: LeaderPortraitKey, now: Instant) -> bool {
        self.leader_retry_state
            .get(&key)
            .map(|state| !state.permanent && state.next_retry_at <= now)
            .unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_bundled_map_br_present() {
        assert!(sow_core::maps::bundled_map_br("world").is_some());
        assert!(sow_core::maps::bundled_map_br("northamerica").is_none());
    }

    #[test]
    fn test_thumbnail_decoding() {
        let bytes = sow_core::repo_asset_bytes!("maps/world/thumbnail.webp");
        assert!(image::load_from_memory(bytes).is_ok());
    }

    #[test]
    fn leader_portrait_assets_present() {
        use crate::ui::asset_loader::{AssetLoader, LeaderPortraitKey};
        use sow_core::player::Leader;
        use std::path::Path;

        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/cdn/leaders");
        for leader in Leader::ALL {
            for mobile in [false, true] {
                let filename =
                    AssetLoader::leader_portrait_filename(LeaderPortraitKey { leader, mobile });
                let path = base.join(&filename);
                assert!(
                    path.is_file(),
                    "missing leader portrait: {}",
                    path.display()
                );
            }
        }
    }
}
