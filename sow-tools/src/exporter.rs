use crate::cli_error;
use crate::openfront_import::refresh_catalog;
use crate::poi_extractor::POISpawn;
use sow_core::map::MapTile;
use sow_core::map_file::{self, MapFile, MapSpawn};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub fn export_map(
    map_name: &str,
    display_name: &str,
    width: u32,
    height: u32,
    terrain: Vec<MapTile>,
    spawns: Vec<POISpawn>,
    single_player_config: bool,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = maps_root().join(map_name);
    let map_bin = output_dir.join("map.bin");
    if map_bin.exists() && !force {
        return Err(cli_error::user_error(
            "export map",
            format!(
                "{} already exists; re-run with --force to overwrite",
                map_bin.display()
            ),
        )
        .into());
    }
    fs::create_dir_all(&output_dir)?;

    let terrain_bytes: Vec<u8> = terrain.iter().map(|t| t.as_byte()).collect();
    let num_land = terrain_bytes.iter().filter(|b| (*b & 0x80) != 0).count() as u32;
    let map_spawns: Vec<MapSpawn> = spawns
        .iter()
        .map(|s| MapSpawn {
            name: s.name.clone(),
            flag: "xx".to_string(),
            x: s.x,
            y: s.y,
        })
        .collect();

    let map_file = MapFile {
        display_name: display_name.to_string(),
        width,
        height,
        num_land_tiles: num_land,
        spawns: map_spawns,
        terrain: terrain_bytes,
    };
    let encoded = map_file::encode(&map_file);
    fs::write(&map_bin, &encoded)?;

    let mut out = Vec::new();
    let mut writer = brotli::CompressorWriter::new(&mut out, 4096, 11, 22);
    writer.write_all(&encoded)?;
    writer.flush()?;
    drop(writer);
    fs::write(output_dir.join("map.bin.br"), out)?;

    let preview = sow_map::terrain_preview_image(width, height, &map_file.terrain);
    sow_map::write_square_thumbnail(
        &preview,
        &output_dir.join("thumbnail.webp"),
    )
    .map_err(|e| cli_error::user_error("write thumbnail", e))?;

    if let Err(e) = refresh_catalog(&maps_root()) {
        eprintln!(
            "Warning: {}",
            cli_error::user_error("refresh catalog", e)
        );
    }

    if single_player_config {
        let ron_path = "crates/client/assets/configs/default_single_player.ron";
        let bot_count = spawns.len();
        let ron_content = format!(
            r#"(
    max_players: 1,
    bot_count: {bot_count},
    map_name: "{map_name}",
    map_width: {width},
    map_height: {height},
    random_spawn: false,
)"#
        );
        fs::write(ron_path, ron_content)?;
        println!(
            "Wrote default_single_player.ron for {} with {} bots",
            map_name, bot_count
        );
    }

    Ok(())
}

fn maps_root() -> PathBuf {
    std::env::var("SOW_MAPS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("assets/maps"))
}
