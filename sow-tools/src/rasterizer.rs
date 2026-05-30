use serde_json::Value;
use sow_core::map::MapTile;

const OCEAN_WATER: u8 = 0b00100000;
const LAND_PLAINS: u8 = 0b10000000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FillKind {
    Land,
    Water,
}

struct LabeledRing {
    kind: FillKind,
    points: Vec<(f32, f32)>,
}

pub fn rasterize_map(
    data: &Value,
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
    scale: f64,
) -> (u32, u32, Vec<MapTile>) {
    let mut width = ((max_lon - min_lon) * scale).ceil() as u32;
    let mut height = ((max_lat - min_lat) * scale).ceil() as u32;
    width -= width % 4;
    height -= height % 4;
    width = width.max(4);
    height = height.max(4);

    let size = (width * height) as usize;
    let mut grid = vec![MapTile::from_byte(OCEAN_WATER); size];

    let rings = collect_rings(data, min_lon, min_lat, max_lon, max_lat, scale, width, height);

    for ring in &rings {
        if ring.kind == FillKind::Land {
            fill_polygon(width, height, &ring.points, |idx| {
                grid[idx] = MapTile::from_byte(LAND_PLAINS);
            });
        }
    }
    for ring in &rings {
        if ring.kind == FillKind::Water {
            fill_polygon(width, height, &ring.points, |idx| {
                grid[idx] = MapTile::from_byte(0);
            });
        }
    }

    apply_ocean_and_shoreline(&mut grid, width, height);

    (width, height, grid)
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

fn collect_rings(
    data: &Value,
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
    scale: f64,
    width: u32,
    height: u32,
) -> Vec<LabeledRing> {
    let mut rings = Vec::new();
    let Some(elements) = data.get("elements").and_then(|e| e.as_array()) else {
        return rings;
    };

    for element in elements {
        let elem_type = element.get("type").and_then(|t| t.as_str());
        let tags = element.get("tags");

        match elem_type {
            Some("way") => {
                let Some(geom) = element.get("geometry").and_then(|g| g.as_array()) else {
                    if tags_way_is_coastline(tags) {
                        if let Some(nodes) = way_nodes_from_refs(element, elements) {
                            let projected = project_latlons(
                                &nodes, min_lon, max_lat, scale, width, height,
                            );
                            rasterize_polyline_thick(&mut rings, &projected, FillKind::Land, 3.0);
                        }
                    }
                    continue;
                };
                let points = geometry_to_points(
                    geom,
                    min_lon,
                    min_lat,
                    max_lon,
                    max_lat,
                    scale,
                    width,
                    height,
                );
                if points.len() < 3 {
                    if tags_way_is_coastline(tags) {
                        rasterize_polyline_thick(&mut rings, &points, FillKind::Land, 2.5);
                    }
                    continue;
                }
                if let Some(kind) = classify_way(tags) {
                    rings.push(LabeledRing { kind, points });
                }
            }
            Some("relation") => {
                let Some(kind) = classify_relation(tags) else {
                    continue;
                };
                if let Some(members) = element.get("members").and_then(|m| m.as_array()) {
                    for member in members {
                        if member.get("role").and_then(|r| r.as_str()) != Some("outer") {
                            continue;
                        }
                        let Some(geom) = member.get("geometry").and_then(|g| g.as_array()) else {
                            continue;
                        };
                        let points = geometry_to_points(
                            geom,
                            min_lon,
                            min_lat,
                            max_lon,
                            max_lat,
                            scale,
                            width,
                            height,
                        );
                        if points.len() >= 3 {
                            rings.push(LabeledRing { kind, points });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    rings
}

fn geometry_to_points(
    geom: &[Value],
    min_lon: f64,
    _min_lat: f64,
    _max_lon: f64,
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

fn rasterize_polyline_thick(
    rings: &mut Vec<LabeledRing>,
    projected: &[(f32, f32)],
    kind: FillKind,
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
            rings.push(LabeledRing {
                kind,
                points: vec![
                    (cx - r, cy - r),
                    (cx + r, cy - r),
                    (cx + r, cy + r),
                    (cx - r, cy + r),
                ],
            });
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
    if tags.get("natural").and_then(|v| v.as_str()) == Some("coastline") {
        return Some(FillKind::Land);
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
    if tags.get("waterway").is_some() {
        return true;
    }
    false
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
            | Some("coastline")
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
            byte |= 0b00100000;
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
        let center = grid[(10 * width + 10) as usize].is_land();
        let corner = grid[0].is_land();
        assert!(center);
        assert!(!corner);
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
            grid[idx] = MapTile::from_byte(0);
        });
        assert!(grid[(15 * width + 15) as usize].is_water());
        assert!(grid[(2 * width + 2) as usize].is_land());
    }
}
