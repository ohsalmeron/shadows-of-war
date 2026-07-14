#![allow(unused_imports, unused_variables, dead_code)]
use crate::cdn;
use crate::config::DeployConfig;
use crate::deploy::{
    verify_marketing_embed, verify_play_host,
    verify_sitemap,
};
use crate::gcp::{GcpConfig, SyncOpts};
use crate::infra::{self, ServerArtifacts, ServerShipResult};
use crate::package::{self, Profile};
use crate::paths::{remote_data_prod, remote_data_ptr, remote_maps_prod, remote_maps_ptr, Paths};
use crate::wasm;
use anyhow::Result;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseTarget {
    Cg,
    Prod,
    Ptr,
}

impl ReleaseTarget {
    fn profile(self) -> Profile {
        match self {
            ReleaseTarget::Cg => Profile::Crazygames,
            ReleaseTarget::Prod | ReleaseTarget::Ptr => Profile::SelfHosted,
        }
    }

    fn out_dir(self, paths: &Paths) -> &Path {
        match self {
            ReleaseTarget::Cg => &paths.dist_crazygames,
            ReleaseTarget::Prod => &paths.dist_play,
            ReleaseTarget::Ptr => &paths.dist_ptr,
        }
    }

    fn sync_cdn(self) -> bool {
        matches!(
            self,
            ReleaseTarget::Cg | ReleaseTarget::Prod | ReleaseTarget::Ptr
        )
    }

    fn remote_ship(self) -> bool {
        !matches!(self, ReleaseTarget::Cg)
    }

    fn server_unit(self) -> Option<&'static str> {
        match self {
            ReleaseTarget::Cg => None,
            ReleaseTarget::Prod => Some("sow-server"),
            ReleaseTarget::Ptr => Some("sow-server-ptr"),
        }
    }

    fn db_unit(self) -> Option<&'static str> {
        match self {
            ReleaseTarget::Cg => None,
            ReleaseTarget::Prod => Some("sow-database"),
            ReleaseTarget::Ptr => Some("sow-database-ptr"),
        }
    }
}

struct ServerCtx {
    unit: &'static str,
    db_unit: &'static str,
    data_dir: String,
    maps_dir: String,
    maps_url: String,
    ws_url: String,
    db_url: String,
}

pub fn run_release(
    paths: &Paths,
    cfg: &DeployConfig,
    target: ReleaseTarget,
    version: &str,
) -> Result<()> {
    let profile = target.profile();
    let out_dir = target.out_dir(paths);

    // Phase 1: local builds in parallel
    println!("==> Phase 1: build");
    let server_artifacts = std::thread::scope(|s| -> Result<Option<ServerArtifacts>> {
        let paths_cdn = paths.clone();
        // CDN prep is cargo-free — run it in parallel with the WASM compile.
        let cdn_h = if target.sync_cdn() {
            Some(s.spawn(move || cdn::prepare(&paths_cdn)))
        } else {
            None
        };
        // WASM and server both invoke cargo and share the package cache.
        // Run them sequentially to avoid "Blocking waiting for file lock" noise.
        wasm::compile(paths, false)?; // prod/deploy: never ship dev tooling
        let server = None;
        if let Some(h) = cdn_h {
            h.join().expect("cdn prep thread panicked")?;
        }
        Ok(server)
    })?;

    // Phase 2: package (depends on WASM)
    println!("==> Phase 2: package (Self-Hosted)");
    package::build_or_skip(paths, profile, out_dir, version, cfg)?;

    if target == ReleaseTarget::Prod {
        println!("==> Phase 2b: package (CrazyGames bundle)");
        package::build_or_skip(
            paths,
            package::Profile::Crazygames,
            &paths.dist_crazygames,
            version,
            cfg,
        )?;
    }

    if !target.remote_ship() {
        // cg: CDN ship + verify only
        println!("==> Phase 3: ship");
        let cdn_shipped = cdn::ship_or_skip(paths, cfg)?;
        if cdn_shipped {
            println!("==> Phase 4: verify");
            cdn::verify_prod_cdn(cfg)?;
            println!("✅ CDN pipeline OK");
        }
        return Ok(());
    }

    if target == ReleaseTarget::Ptr {
        println!("==> Ptr release target is ignored for GCS CDN. Skipping.");
        return Ok(());
    }

    // Phase 3: ship directly to IONOS sow-web jail
    println!("==> Phase 3: ship directly to IONOS sow-web jail");
    let dist = paths.dist_play.clone();
    let root_path = paths.root.clone();

    println!("==> Syncing entire website, WASM, maps, and assets to IONOS sow-web jail via rsync…");
    let src = format!("{}/", dist.to_str().unwrap().trim_end_matches('/'));
    crate::process::run(
        "rsync",
        &[
            "-az",
            "--delete",
            &src,
            "root@74.208.246.177:/zroot/jails/sow-web/var/www/shadowsofwar.io/",
        ],
        Some(&root_path),
    )?;

    println!("✅ sow-web jail local sync OK");
    Ok(())
}

fn server_ctx(
    target: ReleaseTarget,
    cfg: &DeployConfig,
    _gcp: &GcpConfig,
    remote_home: &str,
) -> Result<ServerCtx> {
    let unit = target.server_unit().expect("server unit");
    let db_unit = target.db_unit().expect("db unit");
    let (env_maps, fallback_maps, origin) = match target {
        ReleaseTarget::Prod => (
            "SOW_REMOTE_MAPS_PROD",
            remote_maps_prod(remote_home),
            cfg.site_origin.as_str(),
        ),
        ReleaseTarget::Ptr => (
            "SOW_REMOTE_MAPS_PTR",
            remote_maps_ptr(remote_home),
            cfg.ptr_origin.as_str(),
        ),
        ReleaseTarget::Cg => unreachable!(),
    };
    let (env_workdir, fallback_workdir) = match target {
        ReleaseTarget::Prod => ("SOW_REMOTE_DATA_PROD", remote_data_prod(remote_home)),
        ReleaseTarget::Ptr => ("SOW_REMOTE_DATA_PTR", remote_data_ptr(remote_home)),
        ReleaseTarget::Cg => unreachable!(),
    };
    Ok(ServerCtx {
        unit,
        db_unit,
        data_dir: std::env::var(env_workdir).unwrap_or(fallback_workdir),
        maps_dir: std::env::var(env_maps).unwrap_or(fallback_maps),
        maps_url: cfg.maps_url(origin),
        ws_url: cfg.ws_url(origin),
        db_url: cfg.db_url(origin),
    })
}
