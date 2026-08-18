//! Single asset URL configuration for every client target (native, web, CrazyGames).
//!
//! Strict by design: every endpoint must be declared explicitly (JS globals on
//! wasm, env vars on native). There are NO defaults and NO derivations — a
//! missing endpoint is a packaging/serving bug and must crash the client at
//! boot. A guessed `/api` once routed database traffic to the CrazyGames CDN
//! (403) and silently booted players into the wrong mode; that class of
//! silent misrouting is forbidden here.

#[derive(Clone, Debug)]
pub struct AssetConfig {
    pub maps_base: String,
    pub assets_base: String,
    pub database_base: String,
    /// Deploy timestamp for cache busting CDN UI assets.
    pub cache_bust: String,
}

impl AssetConfig {
    /// Resolve once at boot: explicit JS globals (wasm) or env vars (native).
    /// Missing configuration panics — the client never guesses endpoints.
    pub fn resolve() -> Self {
        let maps_base = require_endpoint("SOW_MAPS_URL");
        let assets_base = require_endpoint("SOW_ASSETS_URL");
        let database_base = require_endpoint("SOW_DATABASE_URL");
        let cache_bust = Self::resolve_cache_bust();
        log::info!(
            "AssetConfig maps={} assets={} database={}",
            maps_base,
            assets_base,
            database_base
        );
        Self {
            maps_base,
            assets_base,
            database_base,
            cache_bust,
        }
    }

    pub fn map_url(&self, map_key: &str, file: &str) -> String {
        format!(
            "{}/{}/{}",
            self.maps_base.trim_end_matches('/'),
            map_key,
            file
        )
    }

    /// Leader portraits on prod CDN (`/assets/cdn/leaders/`).
    pub fn leader_portrait_url(&self, filename: &str) -> String {
        let base = self.assets_base.trim_end_matches('/');
        let path = format!("{base}/cdn/leaders/{filename}");
        if self.cache_bust.is_empty() {
            path
        } else {
            format!("{path}?v={}", self.cache_bust)
        }
    }

    /// Leader rail / HUD avatars on prod CDN (`/assets/cdn/avatars/`).
    pub fn avatar_url(&self, filename: &str) -> String {
        let base = self.assets_base.trim_end_matches('/');
        let path = format!("{base}/cdn/avatars/{filename}");
        if self.cache_bust.is_empty() {
            path
        } else {
            format!("{path}?v={}", self.cache_bust)
        }
    }

    /// Boot loader/splash webp on prod CDN (`/assets/cdn/ui/`).
    pub fn boot_ui_asset_url(&self, filename: &str) -> String {
        let base = self.assets_base.trim_end_matches('/');
        let path = format!("{base}/cdn/ui/{filename}");
        if self.cache_bust.is_empty() {
            path
        } else {
            format!("{path}?v={}", self.cache_bust)
        }
    }

    fn resolve_cache_bust() -> String {
        if let Some(ts) = Self::js_global("SOW_BUILD_TS") {
            if ts != "__BUILD_TS__" && !ts.is_empty() {
                return ts;
            }
        }
        String::new()
    }

    #[cfg(target_arch = "wasm32")]
    fn js_global(name: &str) -> Option<String> {
        let window = web_sys::window()?;
        let val = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str(name)).ok()?;
        val.as_string().filter(|s| !s.is_empty())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn js_global(_name: &str) -> Option<String> {
        None
    }
}

/// Explicit configuration only: JS global (wasm) or env var (native).
/// Missing/empty value = panic. No fallback, no derivation, ever.
pub(crate) fn require_endpoint(name: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(v) = AssetConfig::js_global(name) {
            return v;
        }
    }
    if let Ok(v) = std::env::var(name) {
        if !v.is_empty() {
            return v;
        }
    }
    panic!(
        "SOW endpoint not configured: {name}. Set the JS global (wasm shell boot) \
         or the env var (native). Refusing to guess a fallback."
    );
}
