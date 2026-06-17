use crate::cdn;
use crate::config::DeployConfig;
use crate::deploy::{
    resolve_remote_maps, resolve_remote_workdir, verify_marketing_embed, verify_play_host,
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

    fn ship_marketing(self) -> bool {
        matches!(self, ReleaseTarget::Prod)
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
        let paths_wasm = paths.clone();
        let paths_cdn = paths.clone();
        let paths_srv = paths.clone();
        // CDN prep is cargo-free — run it in parallel with the WASM compile.
        let cdn_h = if target.sync_cdn() {
            Some(s.spawn(move || cdn::prepare(&paths_cdn)))
        } else {
            None
        };
        // WASM and server both invoke cargo and share the package cache.
        // Run them sequentially to avoid "Blocking waiting for file lock" noise.
        wasm::compile(&paths_wasm)?;
        let server = if target.server_unit().is_some() {
            Some(infra::build_server_if_needed(&paths_srv)?)
        } else {
            None
        };
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

    let gcp = cfg.gcp();
    let remote_home = gcp.remote_home(&paths.remote_home_cache())?;
    let server_ctx = server_ctx(target, cfg, &gcp, &remote_home)?;

    // Continuous Deployment: Sync VPS configurations (Nginx and systemd units) if templates were modified
    infra::deploy_configs_if_needed(paths, cfg)?;

    // Phase 3: ship everything in parallel
    println!("==> Phase 3: ship");
    // Print the deployment target banner BEFORE spawning threads so it appears
    // before any SSH output (e.g. mkdir -p) from the sync threads.
    match target {
        ReleaseTarget::Prod => {
            println!(
                "==> Deploying play → {} + marketing → {}",
                cfg.play_domain(),
                cfg.site_domain()
            );
        }
        ReleaseTarget::Ptr => {
            println!("==> Deploying ptr → {}", cfg.ptr_domain());
        }
        ReleaseTarget::Cg => {}
    }
    // Phase 3a: ship everything except CDN in parallel
    // CDN must run AFTER marketing mirror to avoid the mirror wiping CDN assets.
    let server_ship = std::thread::scope(|s| -> Result<ServerShipResult> {
        let shell_sync = match target {
            ReleaseTarget::Prod => {
                let gcp_a = gcp.clone();
                let dist = paths.dist_play.clone();
                let web_play = cfg.web_root_play();
                Some(s.spawn(move || {
                    gcp_a.sync_dir(
                        &dist,
                        &web_play,
                        &SyncOpts {
                            mirror: true,
                            preserve_basenames: vec!["*.bin".into()],
                            exclude_basenames: vec![],
                        },
                    )
                }))
            }
            ReleaseTarget::Ptr => {
                let gcp_a = gcp.clone();
                let dist = paths.dist_ptr.clone();
                let web_ptr = cfg.web_root_ptr();
                Some(s.spawn(move || {
                    gcp_a.sync_dir(
                        &dist,
                        &web_ptr,
                        &SyncOpts {
                            mirror: true,
                            preserve_basenames: vec!["*.bin".into()],
                            exclude_basenames: vec![],
                        },
                    )
                }))
            }
            ReleaseTarget::Cg => None,
        };

        let site_sync = if target.ship_marketing() {
            let gcp_b = gcp.clone();
            let site = paths.site_web.clone();
            let web_main = cfg.web_root_main();
            Some(s.spawn(move || {
                gcp_b.sync_dir(
                    &site,
                    &web_main,
                    &SyncOpts {
                        mirror: true,
                        preserve_basenames: vec!["assets".into()],
                        ..SyncOpts::default()
                    },
                )
            }))
        } else {
            None
        };

        let maps_dir = server_ctx.maps_dir.clone();
        let gcp_f = gcp.clone();
        let maps = paths.assets_maps.clone();
        let maps_sync = s.spawn(move || {
            gcp_f.sync_dir(
                &maps,
                maps_dir.trim_end_matches('/'),
                &SyncOpts {
                    exclude_basenames: vec![
                        "map.bin".into(),
                        "mini_map.bin".into(),
                        "manifest.json".into(),
                        "maps.json".into(),
                    ],
                    ..SyncOpts::default()
                },
            )
        });

        let paths_srv = paths.clone();
        let gcp_srv = gcp.clone();
        let data_dir = server_ctx.data_dir.clone();
        let unit = server_ctx.unit;
        let version = version.to_string();
        let artifacts = server_artifacts
            .clone()
            .expect("server artifacts required for prod/ptr");
        let server_ship = s.spawn(move || {
            infra::ship_server(&paths_srv, &gcp_srv, &data_dir, &artifacts, &version, unit)
        });

        if let Some(h) = shell_sync {
            h.join().unwrap()?;
        }
        if let Some(h) = site_sync {
            h.join().unwrap()?;
        }
        maps_sync.join().unwrap()?;
        let server_ship = server_ship.join().unwrap()?;
        Ok(server_ship)
    })?;

    // Phase 3b: CDN sync runs AFTER marketing mirror completes.
    // The marketing mirror preserves `/assets/` but CDN must always re-sync
    // to guarantee assets are present after any mirror cleanup.
    let cdn_shipped = cdn::ship_or_skip(paths, cfg)?;

    // Phase 4: finalize + verify
    println!("==> Phase 4: finalize + verify");
    infra::restart_server_if_needed(paths, &gcp, server_ctx.unit, version, &server_ship)?;
    infra::restart_server_if_needed(paths, &gcp, server_ctx.db_unit, version, &server_ship)?;
    if cdn_shipped {
        cdn::verify_prod_cdn(cfg)?;
        println!("✅ CDN pipeline OK");
    }
    match target {
        ReleaseTarget::Prod => {
            verify_play_host(&cfg.play_url())?;
            verify_marketing_embed(&format!("{}/", cfg.site_url()))?;
            verify_sitemap(cfg, &cfg.sitemap_url())?;
        }
        ReleaseTarget::Ptr => verify_play_host(&cfg.ptr_url())?,
        ReleaseTarget::Cg => {}
    }
    infra::verify_server_health(
        &gcp,
        &server_ctx.maps_url,
        &server_ctx.ws_url,
        &server_ctx.db_url,
        server_ctx.unit,
        server_ctx.db_unit,
    )?;
    Ok(())
}

fn server_ctx(
    target: ReleaseTarget,
    cfg: &DeployConfig,
    gcp: &GcpConfig,
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
        data_dir: resolve_remote_workdir(gcp, unit, env_workdir, &fallback_workdir),
        maps_dir: resolve_remote_maps(gcp, unit, env_maps, &fallback_maps),
        maps_url: cfg.maps_url(origin),
        ws_url: cfg.ws_url(origin),
        db_url: cfg.db_url(origin),
    })
}
