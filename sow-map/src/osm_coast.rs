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
    min_lon: f64,
    _min_lat: f64,
    _max_lon: f64,
    max_lat: f64,
    scale: f64,
    width: u32,
    height: u32,
) -> CoastlineGeometry {
    let (_, _, segments) =
        collect_geometry(data, min_lon, _min_lat, _max_lon, max_lat, scale, width, height);
    CoastlineGeometry { segments }
}

pub fn stamp_water_polygons(
    grid: &mut [MapTile],
    data: &Value,
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
    scale: f64,
    width: u32,
    height: u32,
) {
    let (_, water_rings, _) =
        collect_geometry(data, min_lon, min_lat, max_lon, max_lat, scale, width, height);
    for ring in &water_rings {
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

fn collect_geometry(
    data: &Value,
    min_lon: f64,
    _min_lat: f64,
    _max_lon: f64,
    max_lat: f64,
    scale: f64,
    width: u32,
    height: u32,
) -> (Vec<LabeledRing>, Vec<LabeledRing>, Vec<Vec<(f32, f32)>>) {
    let mut land_rings = Vec::new();
    let mut water_rings = Vec::new();
    let mut coastlines = Vec::new();

    let Some(elements) = data.get("elements").and_then(|e| e.as_array()) else {
        return (land_rings, water_rings, coastlines);
    };

    for element in elements {
        let elem_type = element.get("type").and_then(|t| t.as_str());
        let tags = element.get("tags");

        match elem_type {
            Some("way") => {
                if tags_way_is_coastline(tags) {
                    if let Some(geom) = element.get("geometry").and_then(|g| g.as_array()) {
                        let points = geometry_to_points(
                            geom, min_lon, max_lat, scale, width, height,
                        );
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
                let points = geometry_to_points(
                    geom, min_lon, max_lat, scale, width, height,
                );
                if points.len() < 3 {
                    continue;
                }
                if let Some(kind) = classify_way(tags) {
                    let ring = LabeledRing { points };
                    match kind {
                        FillKind::Land => land_rings.push(ring),
                        FillKind::Water => water_rings.push(ring),
                    }
                }
            }
            Some("relation") => {
                let Some(kind) = classify_relation(tags) else {
                    continue;
                };
                if tags
                    .and_then(|t| t.get("natural"))
                    .and_then(|v| v.as_str())
                    == Some("coastline")
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
                        let points = geometry_to_points(
                            geom, min_lon, max_lat, scale, width, height,
                        );
                        if points.len() >= 3 {
                            let ring = LabeledRing { points };
                            match kind {
                                FillKind::Land => land_rings.push(ring),
                                FillKind::Water => water_rings.push(ring),
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    (land_rings, water_rings, coastlines)
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

fn rasterize_polyline_barrier(
    barriers: &mut [bool],
    width: u32,
    height: u32,
    projected: &[(f32, f32)],
    thickness: f32,
) {
    if projected.len() < 2 {
        return;
    }
    for w in 0..projected.len().saturating_sub(1) {
        let (x0, y0) = projected[w];
        let (x1, y1) = projected[w + 1];
        let steps = ((x1 - x0).hypot(y1 - y0).ceil() as i32).max(1);
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let cx = x0 + (x1 - x0) * t;
            let cy = y0 + (y1 - y0) * t;
            let r = thickness;
            for dy in -r.ceil() as i32..=r.ceil() as i32 {
                for dx in -r.ceil() as i32..=r.ceil() as i32 {
                    if (dx * dx + dy * dy) as f32 > r * r {
                        continue;
                    }
                    let px = cx.round() as i32 + dx;
                    let py = cy.round() as i32 + dy;
                    if px >= 0 && py >= 0 && px < width as i32 && py < height as i32 {
                        barriers[(py as u32 * width + px as u32) as usize] = true;
                    }
                }
            }
        }
    }
}

fn dilate_barriers(barriers: &mut [bool], width: u32, height: u32) {
    let w = width as usize;
    let h = height as usize;
    let orig = barriers.to_vec();
    for y in 0..h {
        for x in 0..w {
            if !orig[y * w + x] {
                continue;
            }
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && ny >= 0 && nx < w as i32 && ny < h as i32 {
                        barriers[(ny as usize * w + nx as usize) as usize] = true;
                    }
                }
            }
        }
    }
}

fn flood_fill_land(
    grid: &mut [MapTile],
    width: u32,
    height: u32,
    seed_x: i32,
    seed_y: i32,
    barriers: &[bool],
) {
    let w = width as i32;
    let h = height as i32;
    if seed_x < 0 || seed_y < 0 || seed_x >= w || seed_y >= h {
        return;
    }
    let start = (seed_y * w + seed_x) as usize;
    if barriers.get(start).copied().unwrap_or(false) || is_exterior_ocean(grid[start]) {
        return;
    }

    let mut visited = vec![false; grid.len()];
    let mut stack = vec![start];
    visited[start] = true;

    while let Some(cur) = stack.pop() {
        if is_exterior_ocean(grid[cur]) {
            continue;
        }
        grid[cur] = MapTile::from_byte(LAND_PLAINS);
        let cx = (cur as i32) % w;
        let cy = (cur as i32) / w;
        for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
            let nx = cx + dx;
            let ny = cy + dy;
            if nx < 0 || ny < 0 || nx >= w || ny >= h {
                continue;
            }
            let ni = (ny * w + nx) as usize;
            if visited[ni]
                || barriers.get(ni).copied().unwrap_or(false)
                || is_exterior_ocean(grid[ni])
            {
                continue;
            }
            visited[ni] = true;
            stack.push(ni);
        }
    }
}

fn is_exterior_ocean(tile: MapTile) -> bool {
    tile.as_byte() & OCEAN_WATER != 0
}

fn flood_ocean_from_edges(grid: &mut [MapTile], width: u32, height: u32, barriers: &[bool]) {
    let w = width as usize;
    let h = height as usize;
    let mut visited = vec![false; grid.len()];
    let mut stack = Vec::new();

    for x in 0..w {
        for &y in &[0, h - 1] {
            let idx = y * w + x;
            if !grid[idx].is_land() && !barriers[idx] {
                visited[idx] = true;
                stack.push(idx);
            }
        }
    }
    for y in 0..h {
        for &x in &[0, w - 1] {
            let idx = y * w + x;
            if !grid[idx].is_land() && !visited[idx] && !barriers[idx] {
                visited[idx] = true;
                stack.push(idx);
            }
        }
    }

    while let Some(cur) = stack.pop() {
        if grid[cur].is_land() {
            continue;
        }
        grid[cur] = MapTile::from_byte(OCEAN_WATER);
        let cx = (cur % w) as i32;
        let cy = (cur / w) as i32;
        for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
            let nx = cx + dx;
            let ny = cy + dy;
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                continue;
            }
            let ni = (ny as usize * w + nx as usize) as usize;
            if visited[ni] || barriers[ni] || grid[ni].is_land() {
                continue;
            }
            visited[ni] = true;
            stack.push(ni);
        }
    }
}

fn tags_way_is_coastline(tags: Option<&Value>) -> bool {
    tags.and_then(|t| t.get("natural"))
        .and_then(|v| v.as_str())
        == Some("coastline")
}

fn classify_way(tags: Option<&Value>) -> Option<FillKind> {
    let tags = tags?;
    if is_water_tag(tags) {
        return Some(FillKind::Water);
    }
    if is_land_tag(tags) {
        return Some(FillKind::Land);
    }
    None
}

fn classify_relation(tags: Option<&Value>) -> Option<FillKind> {
    let tags = tags?;
    if is_water_tag(tags) {
        return Some(FillKind::Water);
    }
    None
}

fn is_water_tag(tags: &Value) -> bool {
    let natural = tags.get("natural").and_then(|v| v.as_str());
    if matches!(
        natural,
        Some("water") | Some("bay") | Some("strait") | Some("wetland")
    ) {
        return true;
    }
    if tags.get("landuse").and_then(|v| v.as_str()) == Some("water") {
        return true;
    }
    tags.get("waterway").is_some()
}

fn is_land_tag(tags: &Value) -> bool {
    if is_water_tag(tags) {
        return false;
    }
    if tags.get("landuse").is_some() {
        return true;
    }
    let natural = tags.get("natural").and_then(|v| v.as_str());
    matches!(
        natural,
        Some("wood")
            | Some("grassland")
            | Some("scrub")
            | Some("bare_rock")
            | Some("sand")
            | Some("heath")
            | Some("land")
    )
}

fn fill_polygon<F>(width: u32, height: u32, points: &[(f32, f32)], mut set: F)
where
    F: FnMut(usize),
{
    if points.len() < 3 {
        return;
    }
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    for &(_, y) in points {
        let yi = y.floor() as i32;
        min_y = min_y.min(yi);
        max_y = max_y.max(yi);
    }
    min_y = min_y.max(0);
    max_y = max_y.min(height as i32 - 1);

    for y in min_y..=max_y {
        let scan_y = y as f32 + 0.5;
        let mut intersections = Vec::new();
        let n = points.len();
        for i in 0..n {
            let (x0, y0) = points[i];
            let (x1, y1) = points[(i + 1) % n];
            if (y0 <= scan_y && y1 > scan_y) || (y1 <= scan_y && y0 > scan_y) {
                let t = (scan_y - y0) / (y1 - y0);
                let x = x0 + t * (x1 - x0);
                intersections.push(x);
            }
        }
        intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut i = 0;
        while i + 1 < intersections.len() {
            let x_start = intersections[i].ceil().max(0.0) as i32;
            let x_end = intersections[i + 1].floor().min((width - 1) as f32) as i32;
            for x in x_start..=x_end {
                if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
                    set((y as u32 * width + x as u32) as usize);
                }
            }
            i += 2;
        }
    }
}

fn apply_ocean_and_shoreline(grid: &mut [MapTile], width: u32, height: u32) {
    let w = width as usize;
    let h = height as usize;
    if grid.is_empty() {
        return;
    }

    let mut visited = vec![false; grid.len()];
    let mut water_bodies: Vec<Vec<usize>> = Vec::new();

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if visited[idx] || grid[idx].is_land() {
                continue;
            }
            let mut body = Vec::new();
            let mut stack = vec![idx];
            visited[idx] = true;
            while let Some(cur) = stack.pop() {
                body.push(cur);
                let cx = cur % w;
                let cy = cur / w;
                for (dx, dy) in [(0i32, 1), (1, 0), (0, -1), (-1, 0)] {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let ni = ny as usize * w + nx as usize;
                    if !visited[ni] && !grid[ni].is_land() {
                        visited[ni] = true;
                        stack.push(ni);
                    }
                }
            }
            water_bodies.push(body);
        }
    }

    water_bodies.sort_by_key(|b| std::cmp::Reverse(b.len()));
    if let Some(largest) = water_bodies.first() {
        for &idx in largest {
            let mut byte = grid[idx].as_byte();
            byte |= OCEAN_WATER;
            grid[idx] = MapTile::from_byte(byte);
        }
    }

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let is_land = grid[idx].is_land();
            let mut is_shore = false;
            for (dx, dy) in [(0i32, 1), (1, 0), (0, -1), (-1, 0)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    is_shore = true;
                    break;
                }
                let ni = ny as usize * w + nx as usize;
                if grid[ni].is_land() != is_land {
                    is_shore = true;
                    break;
                }
            }
            if is_shore {
                let mut byte = grid[idx].as_byte();
                byte |= 0b01000000;
                grid[idx] = MapTile::from_byte(byte);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_polygon_marks_interior() {
        let width = 20u32;
        let height = 20u32;
        let mut grid = vec![MapTile::from_byte(OCEAN_WATER); (width * height) as usize];
        let tri = vec![(5.0, 5.0), (15.0, 5.0), (10.0, 15.0)];
        fill_polygon(width, height, &tri, |idx| {
            grid[idx] = MapTile::from_byte(LAND_PLAINS);
        });
        assert!(grid[(10 * width + 10) as usize].is_land());
        assert!(!grid[0].is_land());
    }

    #[test]
    fn water_polygon_over_land() {
        let width = 30u32;
        let height = 30u32;
        let mut grid = vec![MapTile::from_byte(LAND_PLAINS); (width * height) as usize];
        let pond = vec![
            (10.0, 10.0),
            (20.0, 10.0),
            (20.0, 20.0),
            (10.0, 20.0),
        ];
        fill_polygon(width, height, &pond, |idx| {
            grid[idx] = MapTile::from_byte(PURE_WATER);
        });
        assert!(grid[(15 * width + 15) as usize].is_water());
        assert!(grid[(2 * width + 2) as usize].is_land());
    }

    #[test]
    fn coastline_encloses_land_and_exterior_stays_water() {
        let width = 40u32;
        let height = 40u32;
        let size = (width * height) as usize;
        let mut grid = vec![MapTile::from_byte(PURE_WATER); size];
        let mut barriers = vec![false; size];

        let coast = vec![
            (10.0, 10.0),
            (30.0, 10.0),
            (30.0, 30.0),
            (10.0, 30.0),
            (10.0, 10.0),
        ];
        rasterize_polyline_barrier(&mut barriers, width, height, &coast, 5.0);
        dilate_barriers(&mut barriers, width, height);

        flood_ocean_from_edges(&mut grid, width, height, &barriers);
        flood_fill_land(&mut grid, width, height, 20, 20, &barriers);
        apply_ocean_and_shoreline(&mut grid, width, height);

        assert!(grid[(20 * width + 20) as usize].is_land());
        assert!(!grid[0].is_land());
        assert!(!grid[(39 * width + 39) as usize].is_land());
    }

    #[test]
    fn seed_fill_without_barriers_cannot_create_inland() {
        let width = 20u32;
        let height = 20u32;
        let mut grid = vec![MapTile::from_byte(PURE_WATER); (width * height) as usize];
        let barriers = vec![false; grid.len()];
        flood_ocean_from_edges(&mut grid, width, height, &barriers);
        flood_fill_land(&mut grid, width, height, 10, 10, &barriers);
        assert!(!grid[(10 * width + 10) as usize].is_land());
        assert!(grid.iter().all(|t| is_exterior_ocean(*t)));
    }
}
