use clap::{Parser, Subcommand};
use std::error::Error;
use std::path::PathBuf;

mod exporter;
mod openfront_import;
mod overpass;
mod poi_extractor;
mod rasterizer;

/// Shadows of War map tooling: OSM generation and OpenFront import.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    sub: Option<Commands>,
    #[command(flatten)]
    generate: GenerateArgs,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Import an OpenFront map folder (image.png + info.json, or map.bin + manifest.json).
    #[command(name = "import-openfront")]
    ImportOpenfront(ImportOpenfrontArgs),
    /// Regenerate assets/maps/catalog.bin from map.bin headers in subfolders.
    #[command(name = "refresh-catalog")]
    RefreshCatalog(RefreshCatalogArgs),
}

#[derive(Parser, Debug)]
struct RefreshCatalogArgs {
    #[arg(long, default_value = "assets/maps")]
    maps_root: PathBuf,
}

/// Generate a map from an OpenStreetMap bounding box.
#[derive(Parser, Debug)]
pub struct GenerateArgs {
    /// Bounding box (min_lon, min_lat, max_lon, max_lat)
    #[arg(short, long, allow_hyphen_values = true)]
    pub bbox: Option<String>,

    /// Output map name (e.g., 'guadalajara')
    #[arg(short, long)]
    pub name: Option<String>,

    /// Scale factor (pixels per degree of longitude)
    #[arg(short, long, default_value_t = 1000.0)]
    pub scale: f64,

    /// Write default_single_player.ron for this map
    #[arg(long, default_value_t = false)]
    pub single_player_config: bool,
}

#[derive(Parser, Debug)]
struct ImportOpenfrontArgs {
    /// OpenFront map folder (contains image.png + info.json or map.bin)
    #[arg(short, long)]
    input: PathBuf,

    /// Output slug under assets/maps (defaults to folder name)
    #[arg(short, long)]
    name: Option<String>,

    /// Maps root directory
    #[arg(long, default_value = "assets/maps")]
    maps_root: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.sub {
        Some(Commands::ImportOpenfront(import)) => {
            openfront_import::run_import(openfront_import::ImportArgs {
                input: import.input,
                name: import.name,
                maps_root: import.maps_root,
            })?;
        }
        Some(Commands::RefreshCatalog(args)) => {
            openfront_import::refresh_catalog(&args.maps_root)?;
            println!("Wrote {}", args.maps_root.join("catalog.bin").display());
        }
        None => {
            let args = cli.generate;
            let bbox = args
                .bbox
                .ok_or("Missing --bbox (use: min_lon,min_lat,max_lon,max_lat)")?;
            let name = args
                .name
                .ok_or("Missing --name for generated map slug")?;
            run_generate(&bbox, &name, args.scale, args.single_player_config).await?;
        }
    }

    Ok(())
}

async fn run_generate(
    bbox: &str,
    name: &str,
    scale: f64,
    single_player_config: bool,
) -> Result<(), Box<dyn Error>> {
    let parts: Vec<&str> = bbox.split(',').collect();
    if parts.len() != 4 {
        return Err("Bounding box must be in format 'min_lon,min_lat,max_lon,max_lat'".into());
    }

    let min_lon: f64 = parts[0].parse()?;
    let min_lat: f64 = parts[1].parse()?;
    let max_lon: f64 = parts[2].parse()?;
    let max_lat: f64 = parts[3].parse()?;

    println!(
        "Generating map '{name}' for bbox [{min_lon}, {min_lat}, {max_lon}, {max_lat}]"
    );

    println!("Fetching data from OpenStreetMap (Overpass API)...");
    let overpass_data = overpass::fetch_bbox(min_lon, min_lat, max_lon, max_lat).await?;

    println!("Rasterizing terrain...");
    let (map_width, map_height, terrain_grid) = rasterizer::rasterize_map(
        &overpass_data,
        min_lon,
        min_lat,
        max_lon,
        max_lat,
        scale,
    );

    let land_count = terrain_grid.iter().filter(|t| t.is_land()).count();
    let water_count = terrain_grid.len() - land_count;
    println!(
        "Rasterized {map_width}x{map_height}: {land_count} land tiles, {water_count} water tiles"
    );

    println!("Extracting place spawns...");
    let spawns = poi_extractor::extract_bots(
        &overpass_data,
        min_lon,
        min_lat,
        max_lon,
        max_lat,
        scale,
        map_width,
        map_height,
    );
    println!("Found {} spawn points", spawns.len());

    println!("Exporting...");
    exporter::export_map(
        name,
        map_width,
        map_height,
        terrain_grid,
        spawns,
        single_player_config,
    )?;

    println!("Generation complete! Saved to assets/maps/{name}");
    Ok(())
}
