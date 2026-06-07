use std::path::PathBuf;

pub const PROD_ASSETS_PATH: &str = "/var/www/shadowsofwar.io/html/assets";
pub const PORTAL_JS: &str = "sow_client.js";
pub const PORTAL_WASM: &str = "sow_client_bg.wasm";

/// SSH user for deploy (`SOW_DEPLOY_USER`, else `$USER`).
pub fn deploy_user() -> String {
    std::env::var("SOW_DEPLOY_USER")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "sow".into())
}

/// SSH host for deploy (`SOW_DEPLOY_HOST`, else `shadowsofwar.io`).
pub fn deploy_host() -> String {
    std::env::var("SOW_DEPLOY_HOST").unwrap_or_else(|_| "shadowsofwar.io".into())
}

fn data_root() -> String {
    std::env::var("SOW_DATA_ROOT").unwrap_or_else(|_| "/var/lib/sow".into())
}

/// Remote prod maps dir (`SOW_REMOTE_MAPS_PROD`, else `$SOW_DATA_ROOT/prod/maps`).
pub fn remote_maps_prod() -> String {
    std::env::var("SOW_REMOTE_MAPS_PROD")
        .unwrap_or_else(|_| format!("{}/prod/maps", data_root()))
}

/// Remote PTR maps dir (`SOW_REMOTE_MAPS_PTR`, else `$SOW_DATA_ROOT/ptr/maps`).
pub fn remote_maps_ptr() -> String {
    std::env::var("SOW_REMOTE_MAPS_PTR")
        .unwrap_or_else(|_| format!("{}/ptr/maps", data_root()))
}

#[derive(Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub assets_static: PathBuf,
    pub assets_maps: PathBuf,
    pub assets_cdn: PathBuf,
    pub shell: PathBuf,
    pub dist_play: PathBuf,
    pub dist_ptr: PathBuf,
    pub dist_crazygames: PathBuf,
    /// Local marketing + embedded game (`dist/site-dev/www`, `dist/site-dev/game`).
    pub dist_site_dev_game: PathBuf,
    pub dist_site_dev_www: PathBuf,
    pub site_web: PathBuf,
    pub version_file: PathBuf,
    pub cargo_target: PathBuf,
    pub wasm_opt_cache: PathBuf,
    pub infra_hash_cache: PathBuf,
}

impl Paths {
    pub fn discover() -> anyhow::Result<Self> {
        let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = crate_dir.join("..").canonicalize()?;
        Ok(Self {
            assets_static: root.join("assets/static"),
            assets_maps: root.join("assets/maps"),
            assets_cdn: root.join("assets/cdn"),
            shell: root.join("sow-web/shell"),
            dist_play: root.join("dist/play"),
            dist_ptr: root.join("dist/ptr"),
            dist_crazygames: root.join("dist/crazygames"),
            dist_site_dev_game: root.join("dist/site-dev/game"),
            dist_site_dev_www: root.join("dist/site-dev/www"),
            site_web: root.join("sow-web/site"),
            version_file: root.join(".version"),
            cargo_target: std::env::var("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| root.join("target")),
            wasm_opt_cache: root.join("dist/.sow-wasm-opt-cache"),
            infra_hash_cache: root.join("dist/.sow-infra-hash"),
            root,
        })
    }

    pub fn infra_hash_cache(&self) -> PathBuf {
        self.infra_hash_cache.clone()
    }

    pub fn wasm_release_input(&self) -> PathBuf {
        self.cargo_target
            .join("wasm32-unknown-unknown/wasm-release/sow_client.wasm")
    }
}
