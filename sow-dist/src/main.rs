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
mod ui_gen;
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
    /// Pack emoji atlas and manifest from scanned Rust sources.
    #[command(name = "emoji", visible_aliases = ["e"])]
    Emoji,
    /// Launch the campaign roster editor — drag tribes on the map, no recompile.
    #[command(name = "map", visible_aliases = ["m", "editor"])]
    Map {
        #[arg(short, long, default_value_t = 8777)]
        port: u16,
    },
    /// Launch the UI scene editor backend — save/export `assets/ui/*.json`, codegen on export.
    #[command(name = "ui", visible_aliases = ["u"])]
    Ui {
        #[arg(short, long, default_value_t = 8778)]
        port: u16,
    },
    /// Spawn bot clients for load testing against a server.
    #[command(name = "bot", visible_aliases = ["b"])]
    Bot {
        /// Number of bots to spawn (default: 5).
        count: Option<usize>,
        /// Server WebSocket URL.
        #[arg(short, long, default_value = "wss://shadowsofwar.io/ws/")]
        url: String,
        /// Passive bots (no random intents).
        #[arg(short, long)]
        passive: bool,
        /// Join a specific lobby ID.
        #[arg(long)]
        lobby_id: Option<u64>,
    },
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
        Command::Emoji => cmd_emoji(&paths),
        Command::Map { port } => serve::serve_campaign_editor(&paths, port),
        Command::Ui { port } => serve::serve_ui_editor(&paths, port),
        Command::Bot {
            count,
            url,
            passive,
            lobby_id,
        } => cmd_bot(&paths, count, url, passive, lobby_id),
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
    build_astro_site(paths)?;
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
    println!("==> local wasm v{version}");
    build_astro_site(paths)?;
    wasm::compile(paths, true)?; // local QA: dev tools available via Settings → Dev Tools
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
    process::wait_for_cargo_unlock(&paths.cargo_target);
    println!("==> Running client (VERBOSE)...");
    process::run_env(
        "cargo",
        &["run", "--bin", "client", "--"],
        Some(&paths.root),
        &[("VERBOSE", "1")],
    )
}

fn cmd_emoji(paths: &Paths) -> Result<()> {
    process::wait_for_cargo_unlock(&paths.cargo_target);
    println!("==> Packing emoji atlas (VERBOSE)...");
    process::run_env(
        "cargo",
        &["run", "--bin", "sow-tools", "--", "pack-emoji-atlas"],
        Some(&paths.root),
        &[("VERBOSE", "1")],
    )
}

fn cmd_bot(
    paths: &Paths,
    count: Option<usize>,
    url: String,
    passive: bool,
    lobby_id: Option<u64>,
) -> Result<()> {
    process::wait_for_cargo_unlock(&paths.cargo_target);
    let n = count.unwrap_or(5);
    let n_str = n.to_string();
    let lid_str = lobby_id.map(|v| v.to_string());

    let mut args: Vec<&str> = vec![
        "run",
        "-p",
        "sow-tools",
        "--bin",
        "bot-manager",
        "--",
        "--url",
        &url,
        "--count",
        &n_str,
    ];
    if passive {
        args.push("--active");
        args.push("false");
    }
    if let Some(ref s) = lid_str {
        args.push("--lobby-id");
        args.push(s);
    }

    println!("==> Spawning {} bot(s) → {}", n, url);
    process::run("cargo", &args, Some(&paths.root))
}

fn build_astro_site(paths: &Paths) -> Result<()> {
    let sow_web_dir = paths.root.join("sow-web");
    if !sow_web_dir.join("package.json").is_file() {
        return Ok(());
    }

    println!("==> Compiling Astro marketing site...");

    let node_modules = sow_web_dir.join("node_modules");
    if !node_modules.is_dir() {
        println!("==> sow-web/node_modules not found. Running npm install...");
        if let Err(e) = process::run("npm", &["install"], Some(&sow_web_dir)) {
            println!("⚠️ Warning: failed to run npm install: {e}. Build might fail if Astro is not installed.");
        }
    }

    if let Err(e) = process::run("npm", &["run", "build"], Some(&sow_web_dir)) {
        anyhow::bail!("Failed to compile Astro site: {e}");
    }

    println!("✅ Astro marketing site compiled successfully!");
    Ok(())
}
