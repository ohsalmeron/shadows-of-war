use std::path::PathBuf;

pub const PORTAL_JS: &str = "sow_client.js";
pub const PORTAL_WASM: &str = "sow_client_bg.wasm";

/// Remote prod server working dir (`$HOME/shadowsofwar` on VPS).
pub fn remote_data_prod(remote_home: &str) -> String {
    std::env::var("SOW_REMOTE_DATA_PROD").unwrap_or_else(|_| format!("{remote_home}/shadowsofwar"))
}

/// Remote PTR server working dir.
pub fn remote_data_ptr(remote_home: &str) -> String {
    std::env::var("SOW_REMOTE_DATA_PTR")
        .unwrap_or_else(|_| format!("{remote_home}/shadowsofwar-ptr"))
}

/// Remote prod maps dir.
pub fn remote_maps_prod(remote_home: &str) -> String {
    std::env::var("SOW_REMOTE_MAPS_PROD")
        .unwrap_or_else(|_| format!("{remote_home}/shadowsofwar/assets/maps"))
}

/// Remote PTR maps dir.
pub fn remote_maps_ptr(remote_home: &str) -> String {
    std::env::var("SOW_REMOTE_MAPS_PTR")
        .unwrap_or_else(|_| format!("{remote_home}/shadowsofwar-ptr/assets/maps"))
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
            wasm_opt_cache: root.join("dist/.sow-state/wasm-opt-cache"),
            infra_hash_cache: root.join("dist/.sow-state/infra-hash"),
            root,
        })
    }

    pub fn infra_hash_cache(&self) -> PathBuf {
        self.infra_hash_cache.clone()
    }

    pub fn deployed_version_cache(&self, unit: &str) -> PathBuf {
        self.state_dir().join(format!("deployed-version-{unit}"))
    }

    pub fn remote_home_cache(&self) -> PathBuf {
        self.state_dir().join("remote-home")
    }

    pub fn dist_root(&self) -> PathBuf {
        self.root.join("dist")
    }

    pub fn state_dir(&self) -> PathBuf {
        self.root.join("dist/.sow-state")
    }

    pub fn deploy_dir(&self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("deploy")
    }

    pub fn deploy_nginx(&self, name: &str) -> PathBuf {
        self.deploy_dir().join("nginx").join(name)
    }

    pub fn deploy_systemd(&self, name: &str) -> PathBuf {
        self.deploy_dir().join("systemd").join(name)
    }

    pub fn wasm_release_input(&self) -> PathBuf {
        self.cargo_target
            .join("wasm32-unknown-unknown/wasm-release/sow_client.wasm")
    }

    pub fn package_hash_cache(&self, key: &str) -> PathBuf {
        self.state_dir().join(format!("package-hash-{key}"))
    }

    pub fn cdn_hash_cache(&self) -> PathBuf {
        self.state_dir().join("cdn-hash")
    }
}
