//! Single asset URL configuration for every client target (native, web, CrazyGames).

const DEFAULT_CDN: &str = "https://shadowsofwar.io";

#[derive(Clone, Debug)]
pub struct AssetConfig {
    pub maps_base: String,
    pub assets_base: String,
    /// Deploy timestamp for cache busting streamed UI assets.
    pub cache_bust: String,
}

impl AssetConfig {
    /// Resolve once at boot: JS globals (wasm) → env vars → CDN defaults.
    pub fn resolve() -> Self {
        let maps_base = Self::resolve_maps_base();
        let assets_base = Self::resolve_assets_base();
        let cache_bust = Self::resolve_cache_bust();
        log::info!(
            "AssetConfig maps={} assets={}",
            maps_base,
            assets_base
        );
        Self {
            maps_base,
            assets_base,
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

    pub fn leader_portrait_url(&self, filename: &str) -> String {
        let base = format!(
            "{}/ui/leaders/{}",
            self.assets_base.trim_end_matches('/'),
            filename
        );
        if self.cache_bust.is_empty() {
            base
        } else {
            format!("{base}?v={}", self.cache_bust)
        }
    }

    fn resolve_maps_base() -> String {
        if let Some(url) = Self::js_global("SOW_MAPS_URL") {
            return url;
        }
        if let Ok(url) = std::env::var("SOW_MAPS_URL") {
            if !url.is_empty() {
                return url;
            }
        }
        if let Ok(ws) = std::env::var("SOW_WS_URL") {
            if let Some(derived) = maps_url_from_ws_url(&ws) {
                return derived;
            }
        }
        format!("{DEFAULT_CDN}/maps")
    }

    fn resolve_assets_base() -> String {
        if let Some(url) = Self::js_global("SOW_ASSETS_URL") {
            return url;
        }
        if let Ok(url) = std::env::var("SOW_ASSETS_URL") {
            if !url.is_empty() {
                return url;
            }
        }
        format!("{DEFAULT_CDN}/assets")
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

fn maps_url_from_ws_url(ws_url: &str) -> Option<String> {
    let rest = ws_url
        .strip_prefix("wss://")
        .or_else(|| ws_url.strip_prefix("ws://"))?;
    let host = rest.split('/').next()?.split(':').next()?;
    if host == "127.0.0.1" || host == "localhost" {
        Some("http://127.0.0.1:25566/maps".to_string())
    } else {
        Some(format!("https://{host}/maps"))
    }
}
