use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sow_core::map_file;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

const LAND_BIT: u8 = 0x80;
const OCEAN_BIT: u8 = 0x20;
const SHORE_BIT: u8 = 0x40;
const SMALL_WATER_LIMIT: usize = 30;

pub struct MapAuditArgs {
    pub maps_root: PathBuf,
    pub map: Option<String>,
    pub json: bool,
}

#[derive(Debug, Serialize)]
struct AuditReport {
    map: String,
    width: u32,
    height: u32,
    cells: usize,
    map_bin_sha256: String,
    map_bin_br_sha256: Option<String>,
    thumbnail_sha256: Option<String>,
    source_present: Option<bool>,
    source_hash_matches: Option<bool>,
    compressed_payload_matches: Option<bool>,
    land_tiles_header: u32,
    land_tiles: usize,
    water_tiles: usize,
    ocean_tiles: usize,
    shoreline_tiles: usize,
    water_components_4: usize,
    tiny_water_components_4_lt30: usize,
    largest_water_component_4: usize,
    water_components_8: usize,
    tiny_water_components_8_lt30: usize,
    largest_water_component_8: usize,
    recipe_present: bool,
    recipe_artifacts_match: Option<bool>,
    recipe_reproducible: Option<bool>,
    valid: bool,
}

#[derive(Debug, Deserialize)]
struct SourcesManifest {
    recipes: Option<std::collections::BTreeMap<String, RecipeRecord>>,
}

#[derive(Debug, Deserialize)]
struct RecipeRecord {
    source_path: String,
    source_sha256: String,
    target_width: u32,
    target_height: u32,
    reproducible: bool,
    map_bin_sha256: String,
    map_bin_br_sha256: String,
    thumbnail_sha256: String,
}

pub fn run(args: MapAuditArgs) -> Result<(), Box<dyn std::error::Error>> {
    let recipes = load_recipes(&args.maps_root)?;
    let mut dirs = fs::read_dir(&args.maps_root)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    dirs.sort();

    let reports = dirs
        .into_iter()
        .filter(|path| path.is_dir())
        .filter(|path| {
            args.map
                .as_deref()
                .is_none_or(|name| path.file_name().and_then(|v| v.to_str()) == Some(name))
        })
        .filter(|path| path.join("map.bin").is_file())
        .map(|path| {
            let key = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("map directory has no UTF-8 name")?;
            audit_map(
                &path,
                args.maps_root
                    .parent()
                    .and_then(Path::parent)
                    .unwrap_or_else(|| Path::new(".")),
                recipes.as_ref().and_then(|all| all.get(key)),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    if reports.is_empty() {
        return Err(format!("no map.bin files found in {}", args.maps_root.display()).into());
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&reports)?);
    } else {
        for report in &reports {
            println!(
                "{} {}x{} land={} water={} ocean={} shore={} water4={} (<30:{}) water8={} (<30:{}) recipe={} valid={}",
                report.map,
                report.width,
                report.height,
                report.land_tiles,
                report.water_tiles,
                report.ocean_tiles,
                report.shoreline_tiles,
                report.water_components_4,
                report.tiny_water_components_4_lt30,
                report.water_components_8,
                report.tiny_water_components_8_lt30,
                report.recipe_artifacts_match.unwrap_or(false),
                report.valid,
            );
        }
    }

    if reports.iter().any(|report| !report.valid) {
        return Err("map audit found invalid map metadata or compressed payload".into());
    }
    Ok(())
}

fn load_recipes(
    maps_root: &Path,
) -> Result<Option<std::collections::BTreeMap<String, RecipeRecord>>, Box<dyn std::error::Error>> {
    let path = maps_root.join("SOURCES.toml");
    if !path.is_file() {
        return Ok(None);
    }
    let manifest: SourcesManifest = toml::from_str(&fs::read_to_string(path)?)?;
    Ok(manifest.recipes)
}

fn audit_map(
    dir: &Path,
    repo_root: &Path,
    recipe: Option<&RecipeRecord>,
) -> Result<AuditReport, Box<dyn std::error::Error>> {
    let map_path = dir.join("map.bin");
    let map_bytes = fs::read(&map_path)?;
    let map = map_file::parse(&map_bytes)?;
    let cells = map.terrain.len();

    let land_tiles = map
        .terrain
        .iter()
        .filter(|&&tile| tile & LAND_BIT != 0)
        .count();
    let water_tiles = cells - land_tiles;
    let ocean_tiles = map
        .terrain
        .iter()
        .filter(|&&tile| tile & OCEAN_BIT != 0 && tile & LAND_BIT == 0)
        .count();
    let shoreline_tiles = map
        .terrain
        .iter()
        .filter(|&&tile| tile & SHORE_BIT != 0)
        .count();

    let water4 = water_components(&map.terrain, map.width, map.height, false);
    let water8 = water_components(&map.terrain, map.width, map.height, true);
    let compressed_payload_matches = compressed_payload_matches(dir, &map_bytes)?;
    let map_bin_sha256 = sha256(&map_bytes);
    let map_bin_br_sha256 = optional_sha256(&dir.join("map.bin.br"))?;
    let thumbnail_sha256 = optional_sha256(&dir.join("thumbnail.webp"))?;
    let (source_present, source_hash_matches) = if let Some(record) = recipe {
        let source_path = resolve_source_path(repo_root, Path::new(&record.source_path));
        let source_hash = optional_sha256(&source_path)?;
        (
            Some(source_path.is_file()),
            Some(source_hash.as_deref() == Some(record.source_sha256.as_str())),
        )
    } else {
        (None, None)
    };
    let recipe_artifacts_match = recipe.map(|record| {
        record.target_width == map.width
            && record.target_height == map.height
            && record.map_bin_sha256 == map_bin_sha256
            && map_bin_br_sha256.as_deref() == Some(record.map_bin_br_sha256.as_str())
            && thumbnail_sha256.as_deref() == Some(record.thumbnail_sha256.as_str())
    });
    let valid = map.num_land_tiles as usize == land_tiles
        && compressed_payload_matches.unwrap_or(true)
        && recipe_artifacts_match.unwrap_or(true)
        && source_present.unwrap_or(true)
        && source_hash_matches.unwrap_or(true);

    Ok(AuditReport {
        map: dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("map directory has no UTF-8 name")?
            .to_string(),
        width: map.width,
        height: map.height,
        cells,
        map_bin_sha256,
        map_bin_br_sha256,
        thumbnail_sha256,
        source_present,
        source_hash_matches,
        compressed_payload_matches,
        land_tiles_header: map.num_land_tiles,
        land_tiles,
        water_tiles,
        ocean_tiles,
        shoreline_tiles,
        water_components_4: water4.count,
        tiny_water_components_4_lt30: water4.tiny,
        largest_water_component_4: water4.largest,
        water_components_8: water8.count,
        tiny_water_components_8_lt30: water8.tiny,
        largest_water_component_8: water8.largest,
        recipe_present: recipe.is_some(),
        recipe_artifacts_match,
        recipe_reproducible: recipe.map(|record| record.reproducible),
        valid,
    })
}

fn resolve_source_path(repo_root: &Path, source_path: &Path) -> PathBuf {
    if source_path.is_absolute() {
        source_path.to_path_buf()
    } else {
        repo_root.join(source_path)
    }
}

fn compressed_payload_matches(
    dir: &Path,
    map_bytes: &[u8],
) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    let path = dir.join("map.bin.br");
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path)?;
    let decoded = map_file::decompress_map_payload(&bytes)?;
    Ok(Some(decoded == map_bytes))
}

fn optional_sha256(path: &Path) -> Result<Option<String>, Box<dyn std::error::Error>> {
    path.is_file()
        .then(|| fs::read(path).map(|bytes| sha256(&bytes)))
        .transpose()
        .map_err(Into::into)
}

fn sha256(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

struct ComponentStats {
    count: usize,
    tiny: usize,
    largest: usize,
}

fn water_components(terrain: &[u8], width: u32, height: u32, diagonal: bool) -> ComponentStats {
    let width = width as usize;
    let height = height as usize;
    let mut visited = vec![false; terrain.len()];
    let mut queue = VecDeque::new();
    let mut count = 0;
    let mut tiny = 0;
    let mut largest = 0;

    for start in 0..terrain.len() {
        if terrain[start] & LAND_BIT != 0 || visited[start] {
            continue;
        }
        count += 1;
        visited[start] = true;
        queue.push_back(start);
        let mut size = 0;
        while let Some(index) = queue.pop_front() {
            size += 1;
            let x = index % width;
            let y = index / width;
            for (dx, dy) in neighbors(diagonal) {
                let nx = x as isize + dx;
                let ny = y as isize + dy;
                if nx < 0 || ny < 0 || nx >= width as isize || ny >= height as isize {
                    continue;
                }
                let neighbor = ny as usize * width + nx as usize;
                if !visited[neighbor] && terrain[neighbor] & LAND_BIT == 0 {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        largest = largest.max(size);
        if size < SMALL_WATER_LIMIT {
            tiny += 1;
        }
    }

    ComponentStats {
        count,
        tiny,
        largest,
    }
}

fn neighbors(diagonal: bool) -> &'static [(isize, isize)] {
    const CARDINAL: [(isize, isize); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    const EIGHT: [(isize, isize); 8] = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (-1, 1),
        (1, -1),
        (-1, -1),
    ];
    if diagonal { &EIGHT } else { &CARDINAL }
}

#[cfg(test)]
mod tests {
    use super::water_components;

    const LAND: u8 = super::LAND_BIT;

    fn fixture(width: usize, height: usize, water: &[(usize, usize)]) -> Vec<u8> {
        let mut terrain = vec![LAND; width * height];
        for &(x, y) in water {
            terrain[y * width + x] = 0;
        }
        terrain
    }

    #[test]
    fn one_tile_river_is_a_small_water_body() {
        let terrain = fixture(5, 1, &[(2, 0)]);
        let stats = water_components(&terrain, 5, 1, false);

        assert_eq!(stats.count, 1);
        assert_eq!(stats.largest, 1);
        assert_eq!(stats.tiny, 1);
    }

    #[test]
    fn lakes_separated_by_land_stay_separate() {
        let water = (0..3)
            .flat_map(|y| [(0, y), (1, y), (3, y), (4, y)])
            .collect::<Vec<_>>();
        let terrain = fixture(5, 3, &water);
        let stats = water_components(&terrain, 5, 3, false);

        assert_eq!(stats.count, 2);
        assert_eq!(stats.largest, 6);
        assert_eq!(stats.tiny, 2);
    }

    #[test]
    fn diagonal_water_is_separate_with_cardinal_connectivity() {
        let terrain = fixture(4, 4, &[(1, 1), (2, 2)]);
        let cardinal = water_components(&terrain, 4, 4, false);
        let diagonal = water_components(&terrain, 4, 4, true);

        assert_eq!(cardinal.count, 2);
        assert_eq!(diagonal.count, 1);
    }

    #[test]
    fn one_tile_island_remains_land_inside_water() {
        let water = [
            (0, 0),
            (1, 0),
            (2, 0),
            (0, 1),
            (2, 1),
            (0, 2),
            (1, 2),
            (2, 2),
        ];
        let terrain = fixture(3, 3, &water);
        let land_tiles = terrain.iter().filter(|&&tile| tile & LAND == LAND).count();
        let water = water_components(&terrain, 3, 3, false);

        assert_eq!(land_tiles, 1);
        assert_eq!(water.count, 1);
        assert_eq!(water.largest, 8);
    }
}
