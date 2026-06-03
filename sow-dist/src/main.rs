mod assets;
mod cdn;
mod deploy;
mod package;
mod paths;
mod process;
mod serve;
mod tools;
mod version;
mod wasm;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use package::Profile;
use paths::Paths;
use std::path::Path;

#[derive(Parser)]
#[command(name = "sow-dist", about = "Shadows of War — build dist/ and deploy")]
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
    /// Package dist/crazygames/ (portal .br + assets/static). Syncs prod CDN in parallel.
    #[command(name = "crazygames", visible_alias = "cg")]
    Crazygames {
        #[command(flatten)]
        opts: VersionOpts,
    },
    /// Deploy play.shadowsofwar.io.
    #[command(name = "play", visible_alias = "p")]
    Play {
        #[command(flatten)]
        opts: VersionOpts,
    },
    /// Deploy ptr.shadowsofwar.io.
    Ptr {
        #[command(flatten)]
        opts: VersionOpts,
    },
    /// Local marketing + iframe embed: build wasm, stage www, serve (one pipeline).
    #[command(name = "localsite", visible_alias = "ls")]
    Localsite {
        #[command(flatten)]
        opts: VersionOpts,
        #[arg(short, long, default_value_t = 8787)]
        port: u16,
        /// Build only; do not start the static server.
        #[arg(long)]
        build_only: bool,
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
    let cli = parse_cli();
    let paths = Paths::discover()?;
    tools::check_build_tools()?;

    match cli.cmd {
        Command::Crazygames { opts } => cmd_crazygames(&paths, opts.version),
        Command::Play { opts } => cmd_play(&paths, opts.version),
        Command::Ptr { opts } => cmd_ptr(&paths, opts.version),
        Command::Localsite {
            opts,
            port,
            build_only,
        } => cmd_localsite(&paths, opts.version, port, build_only),
    }
}

fn resolve_version(paths: &Paths, increment: bool) -> Result<String> {
    if increment {
        version::increment(paths)
    } else {
        version::load(paths)
    }
}

fn run_parallel(paths: &Paths, profile: Profile, out: &Path, version: &str) -> Result<()> {
    let paths_cdn = paths.clone();
    let paths_pkg = paths.clone();
    let out = out.to_path_buf();
    let version = version.to_string();
    let cdn = cdn::start_background(&paths_cdn);
    let pkg = std::thread::spawn(move || {
        wasm::compile(&paths_pkg)?;
        package::build(&paths_pkg, profile, &out, &version)
    });
    cdn.join().expect("cdn thread panicked")?;
    pkg.join().expect("package thread panicked")?;
    Ok(())
}

fn cmd_crazygames(paths: &Paths, increment_version: bool) -> Result<()> {
    let version = resolve_version(paths, increment_version)?;
    run_parallel(paths, Profile::Crazygames, &paths.dist_crazygames, &version)?;
    println!(
        "CrazyGames ready: {} — upload entire folder (no assets/cdn/)",
        paths.dist_crazygames.display()
    );
    Ok(())
}

fn cmd_play(paths: &Paths, increment_version: bool) -> Result<()> {
    let version = resolve_version(paths, increment_version)?;
    run_parallel(paths, Profile::SelfHosted, &paths.dist_play, &version)?;
    deploy::deploy_play(paths)?;
    println!("Play deployed v{version} → https://play.shadowsofwar.io/");
    Ok(())
}

fn cmd_ptr(paths: &Paths, increment_version: bool) -> Result<()> {
    let version = resolve_version(paths, increment_version)?;
    run_parallel(paths, Profile::SelfHosted, &paths.dist_ptr, &version)?;
    deploy::deploy_ptr(paths)?;
    println!("PTR deployed v{version} → https://ptr.shadowsofwar.io/");
    Ok(())
}

fn cmd_localsite(
    paths: &Paths,
    increment_version: bool,
    port: u16,
    build_only: bool,
) -> Result<()> {
    let version = resolve_version(paths, increment_version)?;
    println!("==> localsite v{version} (no CDN sync, no brotli, prod APIs at runtime)");
    wasm::compile(paths)?;
    package::build(
        paths,
        Profile::SiteDev,
        &paths.dist_site_dev_game,
        &version,
    )?;
    package::stage_site_www(paths)?;
    let www = &paths.dist_site_dev_www;
    println!("Localsite ready: {}", www.display());
    if build_only {
        println!("  --build-only: skipped server (re-run without flag to serve)");
        return Ok(());
    }
    println!("  → http://127.0.0.1:{port}/ (iframe embed, prod wss/CDN)");
    serve::serve_site_dev(paths, port)
}
