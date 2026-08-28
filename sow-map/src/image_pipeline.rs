//! OpenFront MapGenerator pipeline: classify encoded RGBA, cleanup, downscale, ocean/shore, pack.
//!
//! Same algorithm as Go `map_generator.go` and `sow-tools image-map`: water when
//! `alpha < 20` or `blue == 106`; land elevation from blue channel 140..200.

use image::RgbaImage;
use sow_core::maps;
use std::collections::VecDeque;

const MIN_ISLAND_SIZE: usize = 30;
const MIN_LAKE_SIZE: usize = 0;

const LAND_BIT: u8 = 0b1000_0000;
const SHORE_BIT: u8 = 0b0100_0000;
const OCEAN_BIT: u8 = 0b0010_0000;

#[derive(Clone)]
struct Terrain {
    width: usize,
    height: usize,
    is_land: Vec<bool>,
    is_ocean: Vec<bool>,
    is_shore: Vec<bool>,
    /// Land: elevation 0..=30. Water: Manhattan distance to land.
    magnitude: Vec<f32>,
}

impl Terrain {
    fn new(width: usize, height: usize) -> Self {
        let n = width * height;
        Self {
            width,
            height,
            is_land: vec![false; n],
            is_ocean: vec![false; n],
            is_shore: vec![false; n],
            magnitude: vec![0.0; n],
        }
    }

    #[inline]
    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }
}

pub struct ImagePipelineResult {
    pub width: u32,
    pub height: u32,
    pub map_data: Vec<u8>,
    pub num_land_tiles: u32,
}

/// Run the MapGenerator pipeline on OpenFront-encoded RGBA pixels.
///
/// When `target_dims` is `Some((w, h))`, downscale to that size (editor OSM path).
/// When `None`, downscale with `mobile_safe_dims` preserving aspect (CLI image-map path).
pub fn generate_from_rgba(
    img: &RgbaImage,
    target_dims: Option<(u32, u32)>,
) -> Result<ImagePipelineResult, String> {
    let src_w = img.width() as usize;
    let src_h = img.height() as usize;
    if src_w == 0 || src_h == 0 {
        return Err("empty image".into());
    }

    log::info!("image_pipeline: source {src_w}x{src_h}");

    let mut full = Terrain::new(src_w, src_h);
    classify_rgba_into(img, &mut full);

    remove_small_islands(&mut full);
    remove_small_lakes(&mut full);

    let (dst_w, dst_h) = match target_dims {
        Some((w, h)) => {
            let w = maps::align_map_dim(w.max(4));
            let h = maps::align_map_dim(h.max(4));
            (w, h)
        }
        None => mobile_safe_dims(src_w as u32, src_h as u32),
    };

    log::info!(
        "image_pipeline: downscale to {dst_w}x{dst_h} ({} pixels)",
        dst_w as u64 * dst_h as u64
    );

    let mut small = downscale(&full, dst_w as usize, dst_h as usize);
    mark_ocean(&mut small);
    let shoreline_waters = process_shore(&mut small);
    process_dist_to_land(&mut small, &shoreline_waters);

    let (map_data, num_land_tiles) = pack(&small);
    Ok(ImagePipelineResult {
        width: dst_w,
        height: dst_h,
        map_data,
        num_land_tiles,
    })
}

fn classify_rgba_into(img: &RgbaImage, terrain: &mut Terrain) {
    for y in 0..terrain.height {
        for x in 0..terrain.width {
            let px = img.get_pixel(x as u32, y as u32).0;
            let blue = px[2];
            let alpha = px[3];
            let i = terrain.idx(x, y);
            if alpha < 20 || blue == 106 {
                terrain.is_land[i] = false;
            } else {
                terrain.is_land[i] = true;
                terrain.magnitude[i] = (blue.clamp(140, 200) as f32 - 140.0) / 2.0;
            }
        }
    }
}

pub fn mobile_safe_dims(src_w: u32, src_h: u32) -> (u32, u32) {
    let w = src_w.max(1) as f64;
    let h = src_h.max(1) as f64;
    let pixel_factor = (maps::MAX_MAP_PIXELS as f64 / (w * h)).sqrt();
    let axis_factor = (maps::MAX_MAP_AXIS as f64 / w).min(maps::MAX_MAP_AXIS as f64 / h);
    let factor = pixel_factor.min(axis_factor).min(1.0);
    let dst_w = maps::align_map_dim((w * factor).floor() as u32);
    let dst_h = maps::align_map_dim((h * factor).floor() as u32);
    (dst_w, dst_h)
}

fn neighbors(x: usize, y: usize, w: usize, h: usize) -> impl Iterator<Item = (usize, usize)> {
    let mut out = Vec::with_capacity(4);
    if x > 0 {
        out.push((x - 1, y));
    }
    if x + 1 < w {
        out.push((x + 1, y));
    }
    if y > 0 {
        out.push((x, y - 1));
    }
    if y + 1 < h {
        out.push((x, y + 1));
    }
    out.into_iter()
}

fn flood_region(t: &Terrain, start: usize, want_land: bool, visited: &mut [bool]) -> Vec<usize> {
    let mut region = Vec::new();
    let mut queue = VecDeque::new();
    queue.push_back(start);
    visited[start] = true;
    while let Some(cur) = queue.pop_front() {
        if t.is_land[cur] != want_land {
            continue;
        }
        region.push(cur);
        let x = cur % t.width;
        let y = cur / t.width;
        for (nx, ny) in neighbors(x, y, t.width, t.height) {
            let ni = ny * t.width + nx;
            if !visited[ni] && t.is_land[ni] == want_land {
                visited[ni] = true;
                queue.push_back(ni);
            }
        }
    }
    region
}

fn remove_small_islands(t: &mut Terrain) {
    let mut visited = vec![false; t.is_land.len()];
    let mut removed = 0usize;
    for start in 0..t.is_land.len() {
        if visited[start] || !t.is_land[start] {
            continue;
        }
        let region = flood_region(t, start, true, &mut visited);
        if region.len() < MIN_ISLAND_SIZE {
            removed += 1;
            for &i in &region {
                t.is_land[i] = false;
                t.magnitude[i] = 0.0;
            }
        }
    }
    log::info!("Removed {removed} islands smaller than {MIN_ISLAND_SIZE} tiles");
}

fn remove_small_lakes(t: &mut Terrain) {
    let mut visited = vec![false; t.is_land.len()];
    let mut bodies: Vec<Vec<usize>> = Vec::new();
    for start in 0..t.is_land.len() {
        if visited[start] || t.is_land[start] {
            continue;
        }
        bodies.push(flood_region(t, start, false, &mut visited));
    }
    bodies.sort_by_key(|b| std::cmp::Reverse(b.len()));
    let mut removed = 0usize;
    for body in bodies.iter().skip(1) {
        if body.len() < MIN_LAKE_SIZE {
            removed += 1;
            for &i in body {
                t.is_land[i] = true;
                t.magnitude[i] = 0.0;
            }
        }
    }
    log::info!("Removed {removed} lakes smaller than {MIN_LAKE_SIZE} tiles");
}

fn downscale(src: &Terrain, dst_w: usize, dst_h: usize) -> Terrain {
    let mut dst = Terrain::new(dst_w, dst_h);
    let fx = src.width as f64 / dst_w as f64;
    let fy = src.height as f64 / dst_h as f64;
    for ty in 0..dst_h {
        let y0 = (ty as f64 * fy).floor() as usize;
        let y1 = (((ty + 1) as f64 * fy).ceil() as usize)
            .min(src.height)
            .max(y0 + 1);
        for tx in 0..dst_w {
            let x0 = (tx as f64 * fx).floor() as usize;
            let x1 = (((tx + 1) as f64 * fx).ceil() as usize)
                .min(src.width)
                .max(x0 + 1);
            let mut land_count = 0usize;
            let mut total = 0usize;
            let mut mag_sum = 0.0f32;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let si = sy * src.width + sx;
                    total += 1;
                    if src.is_land[si] {
                        land_count += 1;
                        mag_sum += src.magnitude[si];
                    }
                }
            }
            let di = ty * dst_w + tx;
            if land_count * 2 >= total {
                dst.is_land[di] = true;
                dst.magnitude[di] = if land_count > 0 {
                    mag_sum / land_count as f32
                } else {
                    0.0
                };
            } else {
                dst.is_land[di] = false;
            }
        }
    }
    dst
}

fn mark_ocean(t: &mut Terrain) {
    let mut visited = vec![false; t.is_land.len()];
    let mut largest: Vec<usize> = Vec::new();
    for start in 0..t.is_land.len() {
        if visited[start] || t.is_land[start] {
            continue;
        }
        let region = flood_region(t, start, false, &mut visited);
        if region.len() > largest.len() {
            largest = region;
        }
    }
    for &i in &largest {
        t.is_ocean[i] = true;
    }
    log::info!("Marked ocean with {} water tiles", largest.len());
}

fn process_shore(t: &mut Terrain) -> Vec<usize> {
    let mut shoreline_waters = Vec::new();
    for y in 0..t.height {
        for x in 0..t.width {
            let i = t.idx(x, y);
            let mut shore = false;
            for (nx, ny) in neighbors(x, y, t.width, t.height) {
                let ni = ny * t.width + nx;
                if t.is_land[ni] != t.is_land[i] {
                    shore = true;
                    break;
                }
            }
            if shore {
                t.is_shore[i] = true;
                if !t.is_land[i] {
                    shoreline_waters.push(i);
                }
            }
        }
    }
    shoreline_waters
}

fn process_dist_to_land(t: &mut Terrain, shoreline_waters: &[usize]) {
    let mut visited = vec![false; t.is_land.len()];
    let mut queue: VecDeque<(usize, u32)> = VecDeque::new();
    for &i in shoreline_waters {
        visited[i] = true;
        t.magnitude[i] = 0.0;
        queue.push_back((i, 0));
    }
    while let Some((cur, dist)) = queue.pop_front() {
        let x = cur % t.width;
        let y = cur / t.width;
        for (nx, ny) in neighbors(x, y, t.width, t.height) {
            let ni = ny * t.width + nx;
            if !visited[ni] && !t.is_land[ni] {
                visited[ni] = true;
                t.magnitude[ni] = (dist + 1) as f32;
                queue.push_back((ni, dist + 1));
            }
        }
    }
}

fn pack(t: &Terrain) -> (Vec<u8>, u32) {
    let mut data = Vec::with_capacity(t.is_land.len());
    let mut num_land = 0u32;
    for i in 0..t.is_land.len() {
        let mut byte = 0u8;
        if t.is_land[i] {
            byte |= LAND_BIT;
            num_land += 1;
        }
        if t.is_shore[i] {
            byte |= SHORE_BIT;
        }
        if t.is_ocean[i] {
            byte |= OCEAN_BIT;
        }
        let mag = if t.is_land[i] {
            t.magnitude[i].ceil().min(31.0)
        } else {
            (t.magnitude[i] / 2.0).ceil().min(31.0)
        };
        byte |= mag as u8 & 0b0001_1111;
        data.push(byte);
    }
    (data, num_land)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn encoded_coast(w: u32, h: u32) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let px = if x < w / 2 {
                    Rgba([0, 0, 106, 255])
                } else {
                    Rgba([0, 0, 140, 255])
                };
                img.put_pixel(x, y, px);
            }
        }
        img
    }

    #[test]
    fn coast_produces_ocean_and_shore() {
        let img = encoded_coast(40, 40);
        let result = generate_from_rgba(&img, Some((20, 20))).expect("pipeline");
        assert_eq!(result.width, 20);
        assert!(result.num_land_tiles > 0);
        let ocean = result
            .map_data
            .iter()
            .filter(|b| **b & OCEAN_BIT != 0)
            .count();
        let shore = result
            .map_data
            .iter()
            .filter(|b| **b & SHORE_BIT != 0)
            .count();
        assert!(ocean > 0, "expected ocean tiles");
        assert!(shore > 0, "expected shoreline tiles");
    }

    #[test]
    fn plains_land_mag_zero() {
        let mut img = RgbaImage::new(8, 8);
        for p in img.pixels_mut() {
            *p = Rgba([0, 0, 140, 255]);
        }
        let result = generate_from_rgba(&img, Some((8, 8))).expect("pipeline");
        for &b in &result.map_data {
            assert!(b & LAND_BIT != 0);
            assert_eq!(b & 0b0001_1111, 0, "flat plains should have mag 0");
        }
    }

    #[test]
    fn mobile_safe_dims_preserve_aspect_and_budget() {
        let (w, h) = mobile_safe_dims(2800, 1448);
        assert!(w <= maps::MAX_MAP_AXIS && h <= maps::MAX_MAP_AXIS);
        assert!((w as u64) * (h as u64) <= maps::MAX_MAP_PIXELS as u64);
        let src_ar = 2800.0 / 1448.0;
        let dst_ar = w as f64 / h as f64;
        assert!((src_ar - dst_ar).abs() < 0.05);
    }
}
