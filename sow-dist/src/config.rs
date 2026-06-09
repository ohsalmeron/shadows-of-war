use crate::gcp::GcpConfig;
use anyhow::{Context, Result};
use std::env;
use std::path::Path;

const MISSING_ENV: &str = "copy sow-dist/.env.example to sow-dist/.env and set required variables";

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
    pub fn load() -> Result<Self> {
        let gcp_project = env_required("SOW_GCP_PROJECT")?;
        Ok(Self {
            gcp_project,
            gcp_zone: env_or("SOW_GCP_ZONE", "us-central1-a"),
            gcp_instance: env_or("SOW_GCP_INSTANCE", "sow-server"),
            gcp_static_ip: env_or("SOW_GCP_STATIC_IP", "sow-server-ip"),
            site_origin: env_required("SOW_SITE_ORIGIN")?,
            play_origin: env_required("SOW_PLAY_ORIGIN")?,
            ptr_origin: env_required("SOW_PTR_ORIGIN")?,
            certbot_email: env_required("SOW_CERTBOT_EMAIL")?,
            test_instance: env_optional("SOW_GCP_TEST_INSTANCE"),
            test_zone: env_optional("SOW_GCP_TEST_ZONE"),
            test_static_ip: env_optional("SOW_GCP_TEST_STATIC_IP"),
            test_static_ip_region: env_optional("SOW_GCP_TEST_STATIC_IP_REGION"),
        })
    }

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

}

pub fn load_dotenv(repo_root: &Path) {
    let path = repo_root.join("sow-dist").join(".env");
    if path.is_file() {
        let _ = dotenv::from_path(path);
    }
}

fn env_required(key: &str) -> Result<String> {
    env::var(key).with_context(|| format!("{key} is required — {MISSING_ENV}"))
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

pub fn require_remote_config() -> Result<DeployConfig> {
    DeployConfig::load().map_err(|e| {
        eprintln!("{e}");
        e
    })
}
