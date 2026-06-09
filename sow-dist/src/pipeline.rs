use crate::cdn;
use crate::config::DeployConfig;
use crate::deploy::{
    resolve_remote_maps, resolve_remote_workdir, verify_marketing_embed, verify_play_host,
    verify_sitemap,
};
use crate::gcp::GcpConfig;
use crate::infra::{self, ServerArtifacts, ServerShipResult};
use crate::package::{self, Profile};
use crate::paths::{
    remote_data_prod, remote_data_ptr, remote_maps_prod, remote_maps_ptr, Paths,
};
use crate::wasm;
use anyhow::Result;
use std::path::Path;

#[derive(Clone, Copy, Debug)]
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

    fn out_dir<'a>(self, paths: &'a Paths) -> &'a Path {
        match self {
            ReleaseTarget::Cg => &paths.dist_crazygames,
            ReleaseTarget::Prod => &paths.dist_play,
            ReleaseTarget::Ptr => &paths.dist_ptr,
        }
    }

    fn sync_cdn(self) -> bool {
        matches!(self, ReleaseTarget::Cg | ReleaseTarget::Prod | ReleaseTarget::Ptr)
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
}

struct ServerCtx {
    unit: &'static str,
    data_dir: String,
    maps_dir: String,
    maps_url: String,
    ws_url: String,
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
        let wasm_h = s.spawn(move || wasm::compile(&paths_wasm));
        let cdn_h = if target.sync_cdn() {
            Some(s.spawn(move || cdn::prepare(&paths_cdn)))
        } else {
            None
        };
        let srv_h = if target.server_unit().is_some() {
            Some(s.spawn(move || infra::build_server_if_needed(&paths_srv)))
        } else {
            None
        };
        wasm_h.join().expect("wasm thread panicked")?;
        if let Some(h) = cdn_h {
            h.join().expect("cdn prep thread panicked")?;
        }
        let server = match srv_h {
            Some(h) => Some(h.join().expect("server build thread panicked")?),
            None => None,
        };
        Ok(server)
    })?;

    // Phase 2: package (depends on WASM)
    println!("==> Phase 2: package");
    package::build_or_skip(paths, profile, out_dir, version, cfg)?;

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

    // Phase 3: ship everything in parallel
    println!("==> Phase 3: ship");
    let (cdn_shipped, server_ship) = std::thread::scope(|s| -> Result<(bool, ServerShipResult)> {
        let paths_cdn = paths.clone();
        let cfg_cdn = cfg.clone();
        let cache = paths.dist_root();
        let gcp_cdn = s.spawn(move || cdn::ship_or_skip(&paths_cdn, &cfg_cdn));

        let shell_rsync = match target {
            ReleaseTarget::Prod => {
                let gcp_a = gcp.clone();
                let dist = paths.dist_play.display().to_string();
                let web_play = cfg.web_root_play();
                let cache_a = cache.clone();
                Some(s.spawn(move || {
                    gcp_a.rsync_dir_with_opts(
                        &cache_a,
                        &dist,
                        &web_play,
                        &["-avzL", "--delete", "--exclude=*.bin"],
                    )
                }))
            }
            ReleaseTarget::Ptr => {
                let gcp_a = gcp.clone();
                let dist = paths.dist_ptr.display().to_string();
                let web_ptr = cfg.web_root_ptr();
                let cache_a = cache.clone();
                Some(s.spawn(move || {
                    gcp_a.rsync_dir_with_opts(
                        &cache_a,
                        &dist,
                        &web_ptr,
                        &["-avzL", "--delete", "--exclude=*.bin"],
                    )
                }))
            }
            ReleaseTarget::Cg => None,
        };

        let site_rsync = if target.ship_marketing() {
            let gcp_b = gcp.clone();
            let site = paths.site_web.display().to_string();
            let web_main = cfg.web_root_main();
            let cache_b = cache.clone();
            Some(s.spawn(move || {
                gcp_b.rsync_dir_with_opts(&cache_b, &site, &web_main, &["-avz"])
            }))
        } else {
            None
        };

        let maps_dir = server_ctx.maps_dir.clone();
        let gcp_f = gcp.clone();
        let maps = paths.assets_maps.display().to_string();
        let cache_f = cache.clone();
        let maps_rsync = s.spawn(move || {
            gcp_f.rsync_dir_with_opts(
                &cache_f,
                &maps,
                maps_dir.trim_end_matches('/'),
                &[
                    "-avz",
                    "--exclude=map.bin",
                    "--exclude=mini_map.bin",
                    "--exclude=manifest.json",
                    "--exclude=maps.json",
                ],
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
            infra::ship_server(
                &paths_srv,
                &gcp_srv,
                &data_dir,
                &artifacts,
                &version,
                unit,
            )
        });

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

        if let Some(h) = shell_rsync {
            h.join().unwrap()?;
        }
        if let Some(h) = site_rsync {
            h.join().unwrap()?;
        }
        maps_rsync.join().unwrap()?;
        let server_ship = server_ship.join().unwrap()?;
        let cdn_shipped = gcp_cdn.join().unwrap()?;
        Ok((cdn_shipped, server_ship))
    })?;

    // Phase 4: finalize + verify
    println!("==> Phase 4: finalize");
    gcp.run_remote("sudo restorecon -R /var/www")?;
    infra::restart_server_if_needed(
        paths,
        &gcp,
        server_ctx.unit,
        version,
        &server_ship,
    )?;

    println!("==> Phase 4: verify");
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
        server_ctx.unit,
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
        data_dir: resolve_remote_workdir(gcp, unit, env_workdir, &fallback_workdir),
        maps_dir: resolve_remote_maps(gcp, unit, env_maps, &fallback_maps),
        maps_url: cfg.maps_url(origin),
        ws_url: cfg.ws_url(origin),
    })
}
