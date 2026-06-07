use clap::{Parser, Subcommand};
use std::error::Error;
use std::path::PathBuf;

mod cli_error;
mod exporter;
mod image_map;
mod openfront_import;
mod overpass;
mod poi_extractor;
mod rasterizer;

/// Shadows of War map tooling: OSM generation and OpenFront import.
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about,
    long_about = "Shadows of War map tooling.\n\n\
        With no subcommand, generates a map from an OpenStreetMap bounding box:\n\
        sow-tools --bbox min_lon,min_lat,max_lon,max_lat --name my_map\n\n\
        Override output root with SOW_MAPS_ROOT (default: assets/maps)."
)]
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
    /// Generate a map from a source world-map PNG (land/water by pixel color).
    #[command(name = "image-map")]
    ImageMap(ImageMapArgs),
}

/// Generate a map from a pre-rendered world-map image (no network calls).
#[derive(Parser, Debug)]
struct ImageMapArgs {
    /// Source PNG whose pixel colors encode land/water (OpenFront-style).
    #[arg(short, long)]
    input: PathBuf,

    /// Output map slug under assets/maps.
    #[arg(short, long)]
    name: String,

    /// Human-readable map title stored in map.bin (defaults to --name).
    #[arg(long)]
    display_name: Option<String>,

    /// Optional info.json with nation coordinates for spawns.
    #[arg(long)]
    info: Option<PathBuf>,

    /// Write default_single_player.ron for this map.
    #[arg(long, default_value_t = false)]
    single_player_config: bool,

    /// Overwrite existing map.bin without prompting
    #[arg(long, default_value_t = false)]
    force: bool,
}

#[derive(Parser, Debug)]
struct RefreshCatalogArgs {
    #[arg(long, default_value = "assets/maps")]
    maps_root: PathBuf,
}

/// Generate a map from an OpenStreetMap bounding box.
#[derive(Parser, Debug)]
pub struct GenerateArgs {
    /// Bounding box (min_lon,min_lat,max_lon,max_lat) in WGS84 decimal degrees
    #[arg(short, long, allow_hyphen_values = true)]
    pub bbox: Option<String>,

    /// Output map slug under SOW_MAPS_ROOT / assets/maps (e.g. guadalajara)
    #[arg(short, long)]
    pub name: Option<String>,

    /// Scale factor (pixels per degree of longitude); clamped to mobile-safe max
    #[arg(short, long, default_value_t = 1000.0)]
    pub scale: f64,

    /// Write default_single_player.ron for this map
    #[arg(long, default_value_t = false)]
    pub single_player_config: bool,

    /// Human-readable map title stored in map.bin (defaults to --name)
    #[arg(long)]
    pub display_name: Option<String>,

    /// Overwrite existing map.bin without prompting
    #[arg(long, default_value_t = false)]
    pub force: bool,
}

#[derive(Parser, Debug)]
struct ImportOpenfrontArgs {
    /// OpenFront map folder (contains image.png + info.json or map.bin)
    #[arg(short, long)]
    input: PathBuf,

    /// Output slug under assets/maps (defaults to folder name)
    #[arg(short, long)]
    name: Option<String>,

    /// Maps root directory (also set via SOW_MAPS_ROOT env var in exporter)
    #[arg(long, default_value = "assets/maps")]
    maps_root: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    let result: Result<(), Box<dyn Error>> = match cli.sub {
        Some(Commands::ImportOpenfront(import)) => {
            openfront_import::run_import(openfront_import::ImportArgs {
                input: import.input,
                name: import.name,
                maps_root: import.maps_root,
            })
        }
        Some(Commands::RefreshCatalog(args)) => {
            openfront_import::refresh_catalog(&args.maps_root).map(|_| {
                println!("Wrote {}", args.maps_root.join("catalog.bin").display());
            })
        }
        Some(Commands::ImageMap(args)) => run_image_map(args),
        None => {
            let args = cli.generate;
            let bbox = args
                .bbox
                .ok_or("Missing --bbox (use: min_lon,min_lat,max_lon,max_lat)")?;
            let name = args
                .name
                .ok_or("Missing --name for generated map slug")?;
            run_generate(
                &bbox,
                &name,
                args.display_name.as_deref().unwrap_or(&name),
                args.scale,
                args.single_player_config,
                args.force,
            )
            .await
        }
    };

    if let Err(e) = result {
        cli_error::exit_user_error("command failed", e);
    }
    Ok(())
}

async fn run_generate(
    bbox: &str,
    name: &str,
    display_name: &str,
    scale: f64,
    single_player_config: bool,
    force: bool,
) -> Result<(), Box<dyn Error>> {
    let parts: Vec<&str> = bbox.split(',').collect();
    if parts.len() != 4 {
        return Err("Bounding box must be in format 'min_lon,min_lat,max_lon,max_lat'".into());
    }

    let min_lon: f64 = parts[0].parse()?;
    let min_lat: f64 = parts[1].parse()?;
    let max_lon: f64 = parts[2].parse()?;
    let max_lat: f64 = parts[3].parse()?;

    let max_scale = sow_core::maps::max_scale_for_bbox(min_lon, min_lat, max_lon, max_lat);
    let scale = if scale > max_scale {
        eprintln!(
            "Warning: scale {scale:.2} exceeds mobile-safe max {max_scale:.2}; clamping."
        );
        max_scale
    } else {
        scale
    };
    let (map_width, map_height) =
        sow_core::maps::map_dims_for_bbox(min_lon, min_lat, max_lon, max_lat, scale);
    println!(
        "Target dimensions: {map_width}x{map_height} ({} pixels, max {})",
        map_width as u64 * map_height as u64,
        sow_core::maps::MAX_MAP_PIXELS
    );

    println!(
        "Generating map '{name}' for bbox [{min_lon}, {min_lat}, {max_lon}, {max_lat}] at scale {scale:.2}"
    );

    println!("Fetching coastlines from OpenStreetMap (Overpass API)...");
    let (map_width, map_height) =
        rasterizer::map_dimensions(min_lon, min_lat, max_lon, max_lat, scale);

    let coastlines = overpass::fetch_coastlines_tiled(
        min_lon,
        min_lat,
        max_lon,
        max_lat,
        scale,
        map_width,
        map_height,
        Some(name),
    )
    .await?;

    println!("Rasterizing landmass...");
    let (map_width, map_height, mut terrain_grid) =
        rasterizer::build_landmass_from_coastlines(
            &coastlines,
            min_lon,
            min_lat,
            max_lon,
            max_lat,
            scale,
        );

    println!("Stamping inland water (optional, tile-by-tile)...");
    if let Err(e) = overpass::stamp_water_tiled(
        &mut terrain_grid,
        min_lon,
        min_lat,
        max_lon,
        max_lat,
        scale,
        map_width,
        map_height,
    )
    .await
    {
        eprintln!("Warning: inland water pass failed: {e}");
    }

    let land_count = terrain_grid.iter().filter(|t| t.is_land()).count();
    let water_count = terrain_grid.len() - land_count;
    println!(
        "Rasterized {map_width}x{map_height}: {land_count} land tiles, {water_count} water tiles"
    );
    if land_count == 0 {
        return Err(
            "Rasterizer produced no land tiles (Overpass geometry likely incomplete). Aborting export."
                .into(),
        );
    }

    println!("Extracting place spawns...");
    let places_data = overpass::fetch_places(min_lon, min_lat, max_lon, max_lat).await?;
    let mut spawns = poi_extractor::extract_bots(
        &places_data,
        min_lon,
        min_lat,
        max_lon,
        max_lat,
        scale,
        map_width,
        map_height,
    );
    if spawns.is_empty() {
        eprintln!("Warning: no OSM place nodes; using land-grid fallback spawns");
        spawns = poi_extractor::fallback_spawns_on_land(&terrain_grid, map_width, map_height, 16);
    }
    println!("Found {} spawn points", spawns.len());

    println!("Exporting...");
    exporter::export_map(
        name,
        display_name,
        map_width,
        map_height,
        terrain_grid,
        spawns,
        single_player_config,
        force,
    )?;

    println!("Generation complete! Saved to assets/maps/{name}");
    println!("Map data © OpenStreetMap contributors (ODbL). See https://www.openstreetmap.org/copyright");
    Ok(())
}

fn run_image_map(args: ImageMapArgs) -> Result<(), Box<dyn Error>> {
    let display_name = args.display_name.clone().unwrap_or_else(|| args.name.clone());

    let (src_w, src_h) = image::image_dimensions(&args.input)?;
    println!(
        "Generating '{}' from image {} ({src_w}x{src_h})",
        args.name,
        args.input.display()
    );

    let map = image_map::generate_from_image(&args.input)?;
    let water = map.terrain.len() as u32 - map.num_land_tiles;
    println!(
        "Generated {}x{}: {} land tiles, {} water tiles ({:.1}% land)",
        map.width,
        map.height,
        map.num_land_tiles,
        water,
        100.0 * map.num_land_tiles as f64 / map.terrain.len() as f64
    );
    if map.num_land_tiles == 0 {
        return Err("Image classification produced no land tiles; check the source PNG.".into());
    }

    let spawns = image_map::spawns_from_info(args.info.as_deref(), src_w, src_h, &map);
    println!("Found {} spawn points", spawns.len());

    println!("Exporting...");
    exporter::export_map(
        &args.name,
        &display_name,
        map.width,
        map.height,
        map.terrain,
        spawns,
        args.single_player_config,
        args.force,
    )?;

    println!("Generation complete! Saved to assets/maps/{}", args.name);
    Ok(())
}
