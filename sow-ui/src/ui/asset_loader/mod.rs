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
    /// Avatar CDN fetches (wasm32).
    avatars_fetch_pending: Vec<AvatarFetchKey>,
    pub avatars_in_flight: HashSet<AvatarFetchKey>,
    avatars_fetch_all_queued: bool,
    avatar_retry_state: HashMap<AvatarFetchKey, PortraitRetryState>,
    /// Decoded, cell-sized RGBA avatar portraits for the GPU image atlas. Fed by
    /// `ingest_avatar_webp_bytes` (the single ingestion point for native + web) and kept
    /// **persistently** — sow-client re-uploads any the text renderer is missing each frame, so
    /// portraits survive GPU-renderer teardown/recreation (e.g. tutorial → match).
    pub gpu_avatar_cells: Vec<(AvatarFetchKey, Vec<u8>)>,
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
    use super::{AssetLoader, AvatarFetchKey};
    use sow_core::player::Leader;

    #[test]
    fn avatar_filenames_match_static_tree() {
        assert_eq!(
            AssetLoader::avatar_filename(AvatarFetchKey::Fallback),
            "null.webp"
        );
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

mod avatars;
mod boot;
mod leaders;
mod maps;
mod retry;

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
            leaders_fetch_pending: Vec::new(),
            leaders_in_flight: HashSet::new(),
            boot_ui_fetch_pending: Vec::new(),
            boot_ui_in_flight: HashSet::new(),
            leader_decode_pending: VecDeque::new(),
            leader_portrait_focus: None,
            leader_retry_state: HashMap::new(),
            avatars_fetch_pending: Vec::new(),
            avatars_in_flight: HashSet::new(),
            avatars_fetch_all_queued: false,
            avatar_retry_state: HashMap::new(),
            gpu_avatar_cells: Vec::new(),
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

    pub fn get_assets_to_fetch(
        &mut self,
        lobbies: &[sow_core::protocol::LobbyInfo],
    ) -> Vec<String> {
        let mut maps_to_fetch = Vec::new();

        let mut unique_maps = HashSet::new();
        for l in lobbies {
            unique_maps.insert(l.map_name.clone());
        }

        // Thumbnails: queue missing ones; sow-client drains via poll_thumbnail_fetches.
        for map_name in &unique_maps {
            self.request_thumbnail(map_name);
        }

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

        maps_to_fetch
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

            for key in std::iter::once(AvatarFetchKey::Fallback)
                .chain(Leader::ALL.iter().copied().map(AvatarFetchKey::Leader))
            {
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
}

#[cfg(test)]
mod tests {
    use super::AssetLoader;
    use crate::ui::asset_loader::LeaderPortraitKey;
    use sow_core::player::Leader;
    use std::path::Path;

    #[test]
    fn test_bundled_map_br_present() {
        assert!(sow_core::maps::bundled_map_br("world").is_some());
        assert!(sow_core::maps::bundled_map_br("northamerica").is_none());
    }

    #[test]
    fn test_thumbnail_decoding() {
        let bytes = sow_ui_kit::repo_asset_bytes!("maps/world/thumbnail.webp");
        assert!(image::load_from_memory(bytes).is_ok());
    }

    #[test]
    fn leader_portrait_assets_present() {
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
