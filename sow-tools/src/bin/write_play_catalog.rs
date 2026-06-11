//! Write a minimal `catalog.bin` for portal / play dist (reads `map.bin.br` per key).

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "write-play-catalog")]
struct Args {
    /// Maps root (e.g. assets/maps)
    #[arg(long, default_value = "assets/maps")]
    maps_root: PathBuf,
    /// Output catalog.bin path
    #[arg(short, long)]
    output: PathBuf,
    /// Map folder keys (e.g. world)
    keys: Vec<String>,
}

fn main() {
    let args = Args::parse();
    if args.keys.is_empty() {
        eprintln!("write-play-catalog: at least one map key required");
        std::process::exit(1);
    }

    let mut items = Vec::new();
    for key in &args.keys {
        let slug = sow_core::maps::map_key(key);
        let br_path = args.maps_root.join(&slug).join("map.bin.br");
        let bytes = std::fs::read(&br_path).unwrap_or_else(|e| {
            eprintln!("write-play-catalog: read {}: {e}", br_path.display());
            std::process::exit(1);
        });
        let raw = sow_core::map_file::decompress_map_payload(&bytes).unwrap_or_else(|e| {
            eprintln!("write-play-catalog: decompress {}: {e}", slug);
            std::process::exit(1);
        });
        let header = sow_core::map_file::parse_header(&raw).unwrap_or_else(|e| {
            eprintln!("write-play-catalog: parse header {}: {e}", slug);
            std::process::exit(1);
        });
        items.push((slug, header));
    }

    let catalog = sow_core::map_file::catalog_from_headers(items);
    let encoded = sow_core::map_file::encode_catalog(&catalog);
    if let Some(parent) = args.output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).unwrap_or_else(|e| {
                eprintln!("write-play-catalog: mkdir {}: {e}", parent.display());
                std::process::exit(1);
            });
        }
    }
    std::fs::write(&args.output, &encoded).unwrap_or_else(|e| {
        eprintln!("write-play-catalog: write {}: {e}", args.output.display());
        std::process::exit(1);
    });
    eprintln!(
        "write-play-catalog: {} entries → {} ({} bytes)",
        catalog.entries.len(),
        args.output.display(),
        encoded.len()
    );
}
