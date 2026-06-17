mod assets;
mod cdn;
mod config;
mod deploy;
mod gcp;
mod infra;
mod package;
mod paths;
mod pipeline;
mod process;
mod serve;
mod tools;
mod version;
mod wasm;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use package::Profile;
use paths::Paths;

#[derive(Parser)]
#[command(name = "sow", about = "Shadows of War — build dist/ and deploy")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Args)]
struct VersionOpts {
    /// Increment repo `.version` before build/deploy.
    #[arg(short = 'v', long, visible_aliases = ["v"])]
    version: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Package dist/crazygames/ (portal .br + embedded assets/static). Syncs assets/cdn/ in parallel.
    #[command(name = "crazygames", visible_aliases = ["cg"])]
    Crazygames {
        #[command(flatten)]
        opts: VersionOpts,
    },
    /// Full prod deploy: play shell, marketing site, prod server (not PTR).
    #[command(name = "prod", visible_aliases = ["p", "play"])]
    Prod {
        #[command(flatten)]
        opts: VersionOpts,
    },
    /// Deploy ptr.shadowsofwar.io + PTR server only.
    Ptr {
        #[command(flatten)]
        opts: VersionOpts,
    },
    /// One-time Debian 13 VPS bootstrap on GCP (destroys/recreates sow-server VM).
    #[command(name = "infra")]
    Infra {
        /// Required to delete existing VMs and recreate sow-server on sow-server-ip.
        #[arg(long)]
        confirm_destroy: bool,
        /// Bootstrap an existing sow-server VM (skip delete/create).
        #[arg(long)]
        bootstrap_only: bool,
    },
    /// Local WASM QA: iframe embed, prod wss/CDN at runtime.
    #[command(name = "local", visible_aliases = ["l", "localsite", "ls"])]
    Local {
        #[command(flatten)]
        opts: VersionOpts,
        #[arg(short, long, default_value_t = 8787)]
        port: u16,
        /// Build only; do not start the static server.
        #[arg(long)]
        build_only: bool,
    },
    /// Run native client locally.
    #[command(name = "native", visible_aliases = ["n"])]
    Native,
}

fn normalize_version_argv(args: impl Iterator<Item = String>) -> Vec<String> {
    args.map(|a| {
        if a == "-version" {
            "--version".into()
        } else {
            a
        }
    })
    .collect()
}

fn parse_cli() -> Cli {
    Cli::parse_from(normalize_version_argv(std::env::args()))
}

fn main() -> Result<()> {
    let paths = Paths::discover()?;
    config::load_dotenv(&paths.root);
    let cli = parse_cli();

    match cli.cmd {
        Command::Crazygames { opts } => cmd_crazygames(&paths, opts.version),
        Command::Prod { opts } => cmd_prod(&paths, opts.version),
        Command::Ptr { opts } => cmd_ptr(&paths, opts.version),
        Command::Infra {
            confirm_destroy,
            bootstrap_only,
        } => cmd_infra(&paths, confirm_destroy, bootstrap_only),
        Command::Local {
            opts,
            port,
            build_only,
        } => cmd_local(&paths, opts.version, port, build_only),
        Command::Native => cmd_native(&paths),
    }
}

fn resolve_version(paths: &Paths, increment: bool) -> Result<String> {
    if increment {
        version::increment(paths)
    } else {
        version::load(paths)
    }
}

fn cmd_crazygames(paths: &Paths, increment_version: bool) -> Result<()> {
    let version = resolve_version(paths, increment_version)?;
    let cfg = config::require_deploy_config()?;
    pipeline::run_release(paths, &cfg, pipeline::ReleaseTarget::Cg, &version)?;
    println!(
        "CrazyGames ready: {} — upload entire folder (no assets/cdn/)",
        paths.dist_crazygames.display()
    );
    Ok(())
}

fn cmd_prod(paths: &Paths, increment_version: bool) -> Result<()> {
    let version = resolve_version(paths, increment_version)?;
    let cfg = config::require_deploy_config()?;
    pipeline::run_release(paths, &cfg, pipeline::ReleaseTarget::Prod, &version)?;
    println!(
        "Prod deployed v{version} → {} + {}",
        cfg.play_url(),
        cfg.site_url()
    );
    Ok(())
}

fn cmd_infra(paths: &Paths, confirm_destroy: bool, bootstrap_only: bool) -> Result<()> {
    let cfg = config::require_infra_config()?;
    infra::deploy_infra(paths, &cfg, confirm_destroy, bootstrap_only)
}

fn cmd_ptr(paths: &Paths, increment_version: bool) -> Result<()> {
    let version = resolve_version(paths, increment_version)?;
    let cfg = config::require_deploy_config()?;
    pipeline::run_release(paths, &cfg, pipeline::ReleaseTarget::Ptr, &version)?;
    println!("PTR deployed v{version} → {}", cfg.ptr_url());
    Ok(())
}

fn cmd_local(paths: &Paths, increment_version: bool, port: u16, build_only: bool) -> Result<()> {
    let version = resolve_version(paths, increment_version)?;
    println!("==> local v{version} (no CDN sync, prod wss/CDN at runtime)");
    wasm::compile(paths)?;
    let cfg = config::local_config();
    package::build_or_skip(
        paths,
        Profile::SiteDev,
        &paths.dist_site_dev_game,
        &version,
        &cfg,
    )?;
    package::stage_site_www(paths)?;
    let www = &paths.dist_site_dev_www;
    println!("Local ready: {}", www.display());
    if build_only {
        println!("  --build-only: skipped server (re-run without flag to serve)");
        return Ok(());
    }
    println!("  → http://127.0.0.1:{port}/ (iframe embed, prod wss/CDN)");
    serve::serve_site_dev(paths, port)
}

fn cmd_native(paths: &Paths) -> Result<()> {
    if process::check_any_cargo_lock(&paths.cargo_target) {
        println!("==> Cargo target directory is locked by another process. Waiting for lock...");
    }
    println!("==> Running native client (release, max-perf, VERBOSE)...");
    process::run_env(
        "cargo",
        &[
            "run",
            "--release",
            "--bin",
            "client",
            "--",
        ],
        Some(&paths.root),
        &[
            ("RUSTFLAGS", "-C target-cpu=native"),
            ("VERBOSE", "1"),
        ],
    )
}
