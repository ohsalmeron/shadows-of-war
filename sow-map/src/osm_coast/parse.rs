use serde::{Deserialize, Serialize};
use serde_json::Value;
use sow_core::map::MapTile;

const OCEAN_WATER: u8 = 0b00100000;
const LAND_PLAINS: u8 = 0b10000000;
const PURE_WATER: u8 = 0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FillKind {
    Land,
    Water,
}

struct LabeledRing {
    points: Vec<(f32, f32)>,
}

/// Projected OSM geometry merged across tiles (coastlines only — kept small in memory).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoastlineGeometry {
    pub segments: Vec<Vec<(f32, f32)>>,
}

/// Geographic bounding box in WGS84 degrees, shared by all OSM rasterizer entry points.
#[derive(Clone, Copy)]
pub struct MapBBox {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

pub fn map_dimensions(
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
    scale: f64,
) -> (u32, u32) {
    let width = sow_core::maps::align_map_dim(((max_lon - min_lon) * scale).ceil() as u32);
    let height = sow_core::maps::align_map_dim(((max_lat - min_lat) * scale).ceil() as u32);
    (width, height)
}

pub fn extract_coastlines(
    data: &Value,
    bbox: MapBBox,
    scale: f64,
    width: u32,
    height: u32,
) -> CoastlineGeometry {
    let geo = collect_geometry(data, bbox, scale, width, height);
    CoastlineGeometry {
        segments: geo.coastlines,
    }
}

pub fn stamp_water_polygons(
    grid: &mut [MapTile],
    data: &Value,
    bbox: MapBBox,
    scale: f64,
    width: u32,
    height: u32,
) {
    let geo = collect_geometry(data, bbox, scale, width, height);
    for ring in &geo.water_rings {
        fill_polygon(width, height, &ring.points, |idx| {
            grid[idx] = MapTile::from_byte(PURE_WATER);
        });
    }
}

pub fn build_landmass_from_coastlines(
    coastlines: &CoastlineGeometry,
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
    scale: f64,
) -> (u32, u32, Vec<MapTile>) {
    let (width, height) = map_dimensions(min_lon, min_lat, max_lon, max_lat, scale);
    let size = (width * height) as usize;

    eprintln!(
        "Landmass from {} coastline segments",
        coastlines.segments.len()
    );

    let mut grid = vec![MapTile::from_byte(PURE_WATER); size];
    let mut barriers = vec![false; size];
    for line in &coastlines.segments {
        rasterize_polyline_barrier(&mut barriers, width, height, line, 5.0);
    }
    dilate_barriers(&mut barriers, width, height);

    flood_ocean_from_edges(&mut grid, width, height, &barriers);

    if !coastlines.segments.is_empty() {
        let seed_x = ((min_lon + max_lon) * 0.5 - min_lon) * scale;
        let seed_y = (max_lat - (min_lat + max_lat) * 0.5) * scale;
        flood_fill_land(
            &mut grid,
            width,
            height,
            seed_x.round() as i32,
            seed_y.round() as i32,
            &barriers,
        );
    } else {
        eprintln!("Warning: no coastline data; map will be all ocean");
    }

    let land_count = grid.iter().filter(|t| t.is_land()).count();
    let water_count = grid.len() - land_count;
    eprintln!("Landmass {width}x{height}: {land_count} land, {water_count} water tiles");

    apply_ocean_and_shoreline(&mut grid, width, height);
    (width, height, grid)
}

struct GeometryResult {
    water_rings: Vec<LabeledRing>,
    coastlines: Vec<Vec<(f32, f32)>>,
}

fn collect_geometry(
    data: &Value,
    bbox: MapBBox,
    scale: f64,
    width: u32,
    height: u32,
) -> GeometryResult {
    let min_lon = bbox.min_lon;
    let max_lat = bbox.max_lat;
    let mut water_rings = Vec::new();
    let mut coastlines = Vec::new();

    let Some(elements) = data.get("elements").and_then(|e| e.as_array()) else {
        return GeometryResult {
            water_rings,
            coastlines,
        };
    };

    for element in elements {
        let elem_type = element.get("type").and_then(|t| t.as_str());
        let tags = element.get("tags");

        match elem_type {
            Some("way") => {
                if tags_way_is_coastline(tags) {
                    if let Some(geom) = element.get("geometry").and_then(|g| g.as_array()) {
                        let points =
                            geometry_to_points(geom, min_lon, max_lat, scale, width, height);
                        if points.len() >= 2 {
                            coastlines.push(points);
                        }
                    } else if let Some(nodes) = way_nodes_from_refs(element, elements) {
                        let projected =
                            project_latlons(&nodes, min_lon, max_lat, scale, width, height);
                        if projected.len() >= 2 {
                            coastlines.push(projected);
                        }
                    }
                    continue;
                }

                let Some(geom) = element.get("geometry").and_then(|g| g.as_array()) else {
                    continue;
                };
                let points = geometry_to_points(geom, min_lon, max_lat, scale, width, height);
                if points.len() < 3 {
                    continue;
                }
                if let Some(kind) = classify_way(tags) {
                    let ring = LabeledRing { points };
                    if kind == FillKind::Water {
                        water_rings.push(ring);
                    }
                }
            }
            Some("relation") => {
                let Some(kind) = classify_relation(tags) else {
                    continue;
                };
                if tags.and_then(|t| t.get("natural")).and_then(|v| v.as_str()) == Some("coastline")
                {
                    continue;
                }
                if let Some(members) = element.get("members").and_then(|m| m.as_array()) {
                    for member in members {
                        if member.get("role").and_then(|r| r.as_str()) != Some("outer") {
                            continue;
                        }
                        let Some(geom) = member.get("geometry").and_then(|g| g.as_array()) else {
                            continue;
                        };
                        let points =
                            geometry_to_points(geom, min_lon, max_lat, scale, width, height);
                        if points.len() >= 3 {
                            let ring = LabeledRing { points };
                            if kind == FillKind::Water {
                                water_rings.push(ring);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    GeometryResult {
        water_rings,
        coastlines,
    }
}

fn lon_lat_to_pixel(
    lon: f64,
    lat: f64,
    min_lon: f64,
    max_lat: f64,
    scale: f64,
    width: u32,
    height: u32,
) -> (f32, f32) {
    let x = ((lon - min_lon) * scale) as f32;
    let y = ((max_lat - lat) * scale) as f32;
    (
        x.clamp(0.0, (width.saturating_sub(1)) as f32),
        y.clamp(0.0, (height.saturating_sub(1)) as f32),
    )
}

fn geometry_to_points(
    geom: &[Value],
    min_lon: f64,
    max_lat: f64,
    scale: f64,
    width: u32,
    height: u32,
) -> Vec<(f32, f32)> {
    geom.iter()
        .filter_map(|node| {
            let lat = node.get("lat")?.as_f64()?;
            let lon = node.get("lon")?.as_f64()?;
            Some(lon_lat_to_pixel(
                lon, lat, min_lon, max_lat, scale, width, height,
            ))
        })
        .collect()
}

fn way_nodes_from_refs(way: &Value, elements: &[Value]) -> Option<Vec<(f64, f64)>> {
    let node_map: std::collections::HashMap<i64, (f64, f64)> = elements
        .iter()
        .filter(|e| e.get("type").and_then(|t| t.as_str()) == Some("node"))
        .filter_map(|n| {
            let id = n.get("id")?.as_i64()?;
            let lat = n.get("lat")?.as_f64()?;
            let lon = n.get("lon")?.as_f64()?;
            Some((id, (lat, lon)))
        })
        .collect();

    let refs = way.get("nodes")?.as_array()?;
    let mut points = Vec::new();
    for r in refs {
        let id = r.as_i64()?;
        if let Some((lat, lon)) = node_map.get(&id) {
            points.push((*lat, *lon));
        }
    }
    if points.len() >= 2 {
        Some(points)
    } else {
        None
    }
}

fn project_latlons(
    latlons: &[(f64, f64)],
    min_lon: f64,
    max_lat: f64,
    scale: f64,
    width: u32,
    height: u32,
) -> Vec<(f32, f32)> {
    latlons
        .iter()
        .map(|(lat, lon)| lon_lat_to_pixel(*lon, *lat, min_lon, max_lat, scale, width, height))
        .collect()
}

include!("raster.rs");
include!("flood.rs");
