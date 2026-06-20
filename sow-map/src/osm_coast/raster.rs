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
                        barriers[ny as usize * w + nx as usize] = true;
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
            let ni = ny as usize * w + nx as usize;
            if visited[ni] || barriers[ni] || grid[ni].is_land() {
                continue;
            }
            visited[ni] = true;
            stack.push(ni);
        }
    }
}

fn tags_way_is_coastline(tags: Option<&Value>) -> bool {
    tags.and_then(|t| t.get("natural")).and_then(|v| v.as_str()) == Some("coastline")
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

