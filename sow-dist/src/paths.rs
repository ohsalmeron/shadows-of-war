use std::path::PathBuf;

pub const PROD_USER: &str = "bizkit";
pub const PROD_HOST: &str = "35.239.160.167";
pub const PROD_ASSETS_PATH: &str = "/var/www/shadowsofwar.io/html/assets";
pub const PORTAL_JS: &str = "sow_client.js";
pub const PORTAL_WASM: &str = "sow_client_bg.wasm";

#[derive(Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub assets_static: PathBuf,
    pub assets_cdn: PathBuf,
    pub shell: PathBuf,
    pub dist_play: PathBuf,
    pub dist_ptr: PathBuf,
    pub dist_crazygames: PathBuf,
    pub version_file: PathBuf,
    pub cargo_target: PathBuf,
    pub wasm_opt_cache: PathBuf,
    pub deploy_nginx: PathBuf,
}

impl Paths {
    pub fn discover() -> anyhow::Result<Self> {
        let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = crate_dir.join("..").canonicalize()?;
        Ok(Self {
            assets_static: root.join("assets/static"),
            assets_cdn: root.join("assets/cdn"),
            shell: root.join("sow-web/shell"),
            dist_play: root.join("dist/play"),
            dist_ptr: root.join("dist/ptr"),
            dist_crazygames: root.join("dist/crazygames"),
            version_file: root.join(".version"),
            cargo_target: root.join("target"),
            wasm_opt_cache: root.join("dist/.sow-wasm-opt-cache"),
            deploy_nginx: crate_dir.join("deploy/nginx"),
            root,
        })
    }

    pub fn wasm_release_input(&self) -> PathBuf {
        self.cargo_target
            .join("wasm32-unknown-unknown/wasm-release/sow_client.wasm")
    }
}
