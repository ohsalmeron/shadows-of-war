use crate::gcp::GcpConfig;
use crate::process;
use anyhow::{bail, Context, Result};
use std::env;
use std::path::Path;

const DEFAULT_SITE_ORIGIN: &str = "https://shadowsofwar.io";
const DEFAULT_PLAY_ORIGIN: &str = "https://play.shadowsofwar.io";
const DEFAULT_PTR_ORIGIN: &str = "https://ptr.shadowsofwar.io";

// Internal service ports — override via env if you change them on the VPS.
const DEFAULT_PROD_WS_PORT: &str = "25565";
const DEFAULT_PROD_MAPS_PORT: &str = "25566";
const DEFAULT_PROD_DB_PORT: &str = "25585";
const DEFAULT_PTR_WS_PORT: &str = "25575";
const DEFAULT_PTR_MAPS_PORT: &str = "25576";
const DEFAULT_PTR_DB_PORT: &str = "25595";

const MISSING_GCP_PROJECT: &str =
    "set SOW_GCP_PROJECT or run: gcloud config set project YOUR_PROJECT_ID";

#[derive(Clone, Debug)]
pub struct DeployConfig {
    pub gcp_project: String,
    pub gcp_zone: String,
    pub gcp_instance: String,
    pub gcp_static_ip: String,
    pub site_origin: String,
    pub play_origin: String,
    pub ptr_origin: String,
    pub certbot_email: String,
    pub test_instance: Option<String>,
    pub test_zone: Option<String>,
    pub test_static_ip: Option<String>,
    pub test_static_ip_region: Option<String>,
}

impl DeployConfig {
    pub fn gcp(&self) -> GcpConfig {
        GcpConfig {
            project: self.gcp_project.clone(),
            zone: self.gcp_zone.clone(),
            instance: self.gcp_instance.clone(),
        }
    }

    pub fn site_domain(&self) -> String {
        origin_host(&self.site_origin)
    }

    pub fn play_domain(&self) -> String {
        origin_host(&self.play_origin)
    }

    pub fn ptr_domain(&self) -> String {
        origin_host(&self.ptr_origin)
    }

    pub fn www_site_domain(&self) -> String {
        format!("www.{}", self.site_domain())
    }

    pub fn site_url(&self) -> String {
        trim_origin(&self.site_origin)
    }

    pub fn play_url(&self) -> String {
        format!("{}/", trim_origin(&self.play_origin))
    }

    pub fn ptr_url(&self) -> String {
        format!("{}/", trim_origin(&self.ptr_origin))
    }

    pub fn maps_url(&self, origin: &str) -> String {
        format!("{}/maps/catalog.bin", trim_origin(origin))
    }

    pub fn ws_url(&self, origin: &str) -> String {
        format!("{}/ws/", trim_origin(origin))
    }

    pub fn db_url(&self, origin: &str) -> String {
        format!(
            "{}/api/profile?provider=local&external_id=healthcheck",
            trim_origin(origin)
        )
    }

    pub fn sitemap_url(&self) -> String {
        format!("{}/sitemap.xml", self.site_url())
    }

    pub fn prod_assets_path(&self) -> String {
        format!("/var/www/{}/html/assets", self.site_domain())
    }

    pub fn web_root_main(&self) -> String {
        format!("/var/www/{}/html", self.site_domain())
    }

    pub fn web_root_play(&self) -> String {
        format!("/var/www/{}/html", self.play_domain())
    }

    pub fn web_root_ptr(&self) -> String {
        format!("/var/www/{}/html", self.ptr_domain())
    }

    pub fn prod_ws_port(&self) -> String {
        env_or("SOW_PROD_WS_PORT", DEFAULT_PROD_WS_PORT)
    }

    pub fn prod_maps_port(&self) -> String {
        env_or("SOW_PROD_MAPS_PORT", DEFAULT_PROD_MAPS_PORT)
    }

    pub fn prod_db_port(&self) -> String {
        env_or("SOW_PROD_DB_PORT", DEFAULT_PROD_DB_PORT)
    }

    pub fn ptr_ws_port(&self) -> String {
        env_or("SOW_PTR_WS_PORT", DEFAULT_PTR_WS_PORT)
    }

    pub fn ptr_maps_port(&self) -> String {
        env_or("SOW_PTR_MAPS_PORT", DEFAULT_PTR_MAPS_PORT)
    }

    pub fn ptr_db_port(&self) -> String {
        env_or("SOW_PTR_DB_PORT", DEFAULT_PTR_DB_PORT)
    }
}

/// Public prod origins for `./sow local` — no `.env` or GCP required.
pub fn local_config() -> DeployConfig {
    let (site_origin, play_origin, ptr_origin) = load_origins();
    DeployConfig {
        gcp_project: String::new(),
        gcp_zone: String::new(),
        gcp_instance: String::new(),
        gcp_static_ip: String::new(),
        site_origin,
        play_origin,
        ptr_origin,
        certbot_email: String::new(),
        test_instance: None,
        test_zone: None,
        test_static_ip: None,
        test_static_ip_region: None,
    }
}

/// GCP project + origins for `./sow cg`, `./sow p`, `./sow ptr`.
pub fn require_deploy_config() -> Result<DeployConfig> {
    let cfg = build_deploy_config(None).map_err(|e| {
        eprintln!("{e}");
        e
    })?;
    Ok(cfg)
}

/// Full deploy config for `./sow infra`.
pub fn require_infra_config() -> Result<DeployConfig> {
    // Default to the project contact email; override with SOW_CERTBOT_EMAIL if needed.
    let email = env_or("SOW_CERTBOT_EMAIL", "info@shadowsofwar.io");
    let cfg = build_deploy_config(Some(email)).map_err(|e| {
        eprintln!("{e}");
        e
    })?;
    Ok(cfg)
}

fn build_deploy_config(certbot_email: Option<String>) -> Result<DeployConfig> {
    let (gcp_project, gcp_zone, gcp_instance, gcp_static_ip) = load_gcp_fields()?;
    let (site_origin, play_origin, ptr_origin) = load_origins();
    Ok(DeployConfig {
        gcp_project,
        gcp_zone,
        gcp_instance,
        gcp_static_ip,
        site_origin,
        play_origin,
        ptr_origin,
        certbot_email: certbot_email
            .or_else(|| env_optional("SOW_CERTBOT_EMAIL"))
            .unwrap_or_default(),
        test_instance: env_optional("SOW_GCP_TEST_INSTANCE"),
        test_zone: env_optional("SOW_GCP_TEST_ZONE"),
        test_static_ip: env_optional("SOW_GCP_TEST_STATIC_IP"),
        test_static_ip_region: env_optional("SOW_GCP_TEST_STATIC_IP_REGION"),
    })
}

fn load_gcp_fields() -> Result<(String, String, String, String)> {
    Ok((
        resolve_gcp_project()?,
        env_or("SOW_GCP_ZONE", "us-central1-a"),
        env_or("SOW_GCP_INSTANCE", "sow-server"),
        env_or("SOW_GCP_STATIC_IP", "sow-server-ip"),
    ))
}

fn load_origins() -> (String, String, String) {
    (
        env_or("SOW_SITE_ORIGIN", DEFAULT_SITE_ORIGIN),
        env_or("SOW_PLAY_ORIGIN", DEFAULT_PLAY_ORIGIN),
        env_or("SOW_PTR_ORIGIN", DEFAULT_PTR_ORIGIN),
    )
}

fn resolve_gcp_project() -> Result<String> {
    if let Ok(v) = env::var("SOW_GCP_PROJECT") {
        let v = v.trim().to_string();
        if !v.is_empty() && v != "your-gcp-project-id" {
            return Ok(v);
        }
    }
    let project = process::output("gcloud", &["config", "get-value", "project"])
        .context("could not read gcloud default project")?;
    if project.is_empty() || project == "(unset)" {
        bail!("SOW_GCP_PROJECT is required — {MISSING_GCP_PROJECT}");
    }
    Ok(project)
}

pub fn load_dotenv(repo_root: &Path) {
    let path = repo_root.join("sow-dist").join(".env");
    if path.is_file() {
        let _ = dotenv::from_path(path);
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_optional(key: &str) -> Option<String> {
    let v = env::var(key).ok()?;
    let v = v.trim();
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

fn trim_origin(origin: &str) -> String {
    origin.trim_end_matches('/').to_string()
}

fn origin_host(origin: &str) -> String {
    let s = trim_origin(origin);
    s.strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(&s)
        .split('/')
        .next()
        .unwrap_or(&s)
        .to_string()
}
