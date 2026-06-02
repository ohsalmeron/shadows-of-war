//! Slippy-map tile fetch + land/water classification for the map editor.
//! Native-only (uses ehttp). OSM Standard tiles power preview and in-editor Generate.
//! Headless bbox CLI uses vector Overpass in `osm_overpass.rs`.

use crate::heightmap::WorldHeightmap;
use crossbeam_channel::{Receiver, Sender};
use image::RgbaImage;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::time::Duration;

pub const TILE_SIZE: u32 = 256;
pub const MAX_TILE_ZOOM: u32 = 19;
pub const MAX_TILES_PER_REGION: usize = 144;
pub const OSM_USER_AGENT: &str = "ShadowsOfWar-MapEditor/1.0 (contact: local-dev)";

/// OSM standard map water fill (#aad3df).
const WATER_PALETTE: &[(u8, u8, u8)] = &[(170, 211, 223)];
const COLOR_TOLERANCE: i16 = 32;

static TILE_MSG_TX: OnceLock<Sender<TileMessage>> = OnceLock::new();
static TILE_MSG_RX: OnceLock<Receiver<TileMessage>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub z: u32,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug)]
pub enum TileMessage {
    Ready {
        key: TileKey,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
    Failed {
        key: TileKey,
    },
}

#[derive(Clone, Debug)]
pub enum CachedTile {
    Pending,
    Ready(RgbaImage),
    Failed,
}

pub struct OsmTileCache {
    tiles: HashMap<TileKey, CachedTile>,
    in_flight: HashSet<TileKey>,
    max_entries: usize,
}

impl Default for OsmTileCache {
    fn default() -> Self {
        Self {
            tiles: HashMap::new(),
            in_flight: HashSet::new(),
            max_entries: 512,
        }
    }
}

impl OsmTileCache {
    pub fn get(&self, key: TileKey) -> Option<&CachedTile> {
        self.tiles.get(&key)
    }

    pub fn insert_ready(&mut self, key: TileKey, img: RgbaImage) {
        self.in_flight.remove(&key);
        self.tiles.insert(key, CachedTile::Ready(img));
        self.evict_if_needed();
    }

    pub fn mark_failed(&mut self, key: TileKey) {
        self.in_flight.remove(&key);
        self.tiles.insert(key, CachedTile::Failed);
    }

    pub fn request(&mut self, key: TileKey) {
        if matches!(self.tiles.get(&key), Some(CachedTile::Ready(_))) {
            return;
        }
        if self.in_flight.contains(&key) {
            return;
        }
        self.in_flight.insert(key);
        self.tiles.insert(key, CachedTile::Pending);
        request_tile_async(key);
    }

    pub fn clear(&mut self) {
        self.tiles.clear();
        self.in_flight.clear();
    }

    pub fn retain_zoom(&mut self, z: u32) {
        self.tiles.retain(|k, _| k.z == z);
        self.in_flight.retain(|k| k.z == z);
    }

    pub fn drain_messages(&mut self) {
        let rx = tile_rx();
        while let Ok(msg) = rx.try_recv() {
            match msg {
                TileMessage::Ready {
                    key,
                    rgba,
                    width,
                    height,
                } => {
                    if let Some(img) = RgbaImage::from_raw(width, height, rgba) {
                        self.insert_ready(key, img);
                    } else {
                        self.mark_failed(key);
                    }
                }
                TileMessage::Failed { key } => self.mark_failed(key),
            }
        }
    }

    fn evict_if_needed(&mut self) {
        while self.tiles.len() > self.max_entries {
            if let Some(k) = self
                .tiles
                .keys()
                .find(|k| !self.in_flight.contains(k))
                .copied()
            {
                self.tiles.remove(&k);
            } else {
                break;
            }
        }
    }
}

fn tile_tx() -> &'static Sender<TileMessage> {
    TILE_MSG_TX.get_or_init(|| {
        let (tx, rx) = crossbeam_channel::unbounded();
        TILE_MSG_RX.set(rx).ok();
        tx
    })
}

fn tile_rx() -> &'static Receiver<TileMessage> {
    tile_tx();
    TILE_MSG_RX.get().expect("tile channel initialized")
}

pub fn tile_url(z: u32, x: u32, y: u32) -> String {
    format!("https://tile.openstreetmap.org/{z}/{x}/{y}.png")
}

pub fn lonlat_to_world_px(lon: f64, lat: f64, zoom: u32) -> (f64, f64) {
    let scale = tile_scale(zoom);
    let x = (lon + 180.0) / 360.0 * scale;
    let lat_rad = lat.to_radians();
    let y = (1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * scale;
    (x, y)
}

pub fn world_px_to_lonlat(x: f64, y: f64, zoom: u32) -> (f64, f64) {
    let scale = tile_scale(zoom);
    let lon = x / scale * 360.0 - 180.0;
    let n = std::f64::consts::PI - 2.0 * std::f64::consts::PI * y / scale;
    let lat = n.sinh().atan().to_degrees();
    (lon, lat)
}

pub fn tile_scale(zoom: u32) -> f64 {
    (TILE_SIZE as f64) * 2f64.powi(zoom as i32)
}

pub fn tile_for_world_px(x: f64, y: f64, zoom: u32) -> TileKey {
    let max = 1u32 << zoom;
    let tx = (x / TILE_SIZE as f64).floor().clamp(0.0, (max - 1) as f64) as u32;
    let ty = (y / TILE_SIZE as f64).floor().clamp(0.0, (max - 1) as f64) as u32;
    TileKey {
        z: zoom,
        x: tx,
        y: ty,
    }
}

pub fn tiles_covering_rect(
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    zoom: u32,
) -> Vec<TileKey> {
    let min_x = x0.min(x1);
    let max_x = x0.max(x1);
    let min_y = y0.min(y1);
    let max_y = y0.max(y1);
    let t0 = tile_for_world_px(min_x, min_y, zoom);
    let t1 = tile_for_world_px(max_x, max_y, zoom);
    let mut out = Vec::new();
    for x in t0.x..=t1.x {
        for y in t0.y..=t1.y {
            out.push(TileKey { z: zoom, x, y });
        }
    }
    out
}

fn request_tile_async(key: TileKey) {
    let url = tile_url(key.z, key.x, key.y);
    let tx = tile_tx().clone();
    let mut request = ehttp::Request::get(&url);
    request.headers.insert(
        "User-Agent".to_owned(),
        OSM_USER_AGENT.to_owned(),
    );
    ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
        let send = match result {
            Ok(res) if res.ok => decode_tile_response(&res.bytes).map(|img| {
                let width = img.width();
                let height = img.height();
                TileMessage::Ready {
                    key,
                    rgba: img.into_raw(),
                    width,
                    height,
                }
            }),
            _ => Ok(TileMessage::Failed { key }),
        };
        match send {
            Ok(msg) => {
                let _ = tx.send(msg);
            }
            Err(_) => {
                let _ = tx.send(TileMessage::Failed { key });
            }
        }
    });
}

fn decode_tile_response(bytes: &[u8]) -> Result<RgbaImage, String> {
    image::load_from_memory(bytes)
        .map_err(|e| e.to_string())
        .map(|img| img.to_rgba8())
}

/// Stitch tiles covering a world-px square and crop to it. Blocks until all tiles are ready.
pub fn fetch_region_blocking(
    cache: &mut OsmTileCache,
    zoom: u32,
    world_x: f64,
    world_y: f64,
    world_size: f64,
) -> Result<RgbaImage, String> {
    let keys = tiles_covering_rect(
        world_x,
        world_y,
        world_x + world_size,
        world_y + world_size,
        zoom,
    );
    if keys.len() > MAX_TILES_PER_REGION {
        return Err(format!(
            "Selection needs {} tiles (max {MAX_TILES_PER_REGION}); zoom out or shrink the square",
            keys.len()
        ));
    }

    for &key in &keys {
        cache.request(key);
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    loop {
        cache.drain_messages();
        let all_ready = keys
            .iter()
            .all(|k| matches!(cache.get(*k), Some(CachedTile::Ready(_))));
        if all_ready {
            break;
        }
        let any_failed = keys
            .iter()
            .any(|k| matches!(cache.get(*k), Some(CachedTile::Failed)));
        if any_failed {
            return Err("One or more map tiles failed to download".into());
        }
        if std::time::Instant::now() > deadline {
            return Err("Timed out waiting for map tiles".into());
        }
        for key in &keys {
            if !matches!(cache.get(*key), Some(CachedTile::Ready(_))) {
                cache.request(*key);
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let out_w = world_size.ceil().max(1.0) as u32;
    let out_h = out_w;
    let mut stitched = RgbaImage::new(out_w, out_h);

    for key in keys {
        let CachedTile::Ready(tile) = cache.get(key).cloned().unwrap() else {
            return Err("missing tile after fetch".into());
        };
        let tile_world_x = key.x as f64 * TILE_SIZE as f64;
        let tile_world_y = key.y as f64 * TILE_SIZE as f64;
        for py in 0..tile.height() {
            for px in 0..tile.width() {
                let wx = tile_world_x + px as f64;
                let wy = tile_world_y + py as f64;
                if wx < world_x
                    || wy < world_y
                    || wx >= world_x + world_size
                    || wy >= world_y + world_size
                {
                    continue;
                }
                let ox = (wx - world_x).floor() as u32;
                let oy = (wy - world_y).floor() as u32;
                if ox < out_w && oy < out_h {
                    stitched.put_pixel(ox, oy, *tile.get_pixel(px, py));
                }
            }
        }
    }

    Ok(stitched)
}

fn matches_palette(r: u8, g: u8, b: u8, pr: u8, pg: u8, pb: u8) -> bool {
    (r as i16 - pr as i16).abs() <= COLOR_TOLERANCE
        && (g as i16 - pg as i16).abs() <= COLOR_TOLERANCE
        && (b as i16 - pb as i16).abs() <= COLOR_TOLERANCE
}

/// True when pixel reads as water on OSM standard raster tiles.
pub fn is_water_pixel(r: u8, g: u8, b: u8, a: u8) -> bool {
    if a < 20 {
        return true;
    }
    for &(pr, pg, pb) in WATER_PALETTE {
        if matches_palette(r, g, b, pr, pg, pb) {
            return true;
        }
    }
    false
}

/// Convert stitched OSM tiles to OpenFront MapGenerator pixel encoding.
///
/// Water mask from OSM Standard tiles; land elevation blue channel from `heightmap`.
pub fn classify_osm_to_rgba_with_heightmap(
    img: &RgbaImage,
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
    heightmap: &WorldHeightmap,
) -> RgbaImage {
    let w = img.width();
    let h = img.height();
    let mut out = RgbaImage::new(w, h);
    let lon_span = max_lon - min_lon;
    let lat_span = max_lat - min_lat;

    for y in 0..h {
        let lat = if h <= 1 {
            max_lat
        } else {
            max_lat - (y as f64 / (h - 1) as f64) * lat_span
        };
        for x in 0..w {
            let lon = if w <= 1 {
                min_lon
            } else {
                min_lon + (x as f64 / (w - 1) as f64) * lon_span
            };
            let [r, g, b, a] = img.get_pixel(x, y).0;
            let px = if is_water_pixel(r, g, b, a) {
                [0, 0, 106, 255]
            } else {
                let hm_blue = heightmap.sample_openfront_blue(lon, lat);
                let blue = if hm_blue == 106 {
                    140
                } else {
                    hm_blue.clamp(140, 200)
                };
                [0, 0, blue, 255]
            };
            out.put_pixel(x, y, image::Rgba(px));
        }
    }
    out
}

/// Pick a fetch zoom so the square spans roughly 1024–2048 source pixels.
pub fn pick_fetch_zoom(target_size: u32, world_size_deg: f64) -> u32 {
    let target = target_size.max(256) as f64;
    for z in (3..=18).rev() {
        let scale = tile_scale(z);
        let px_per_deg = scale / 360.0;
        let span_px = world_size_deg * px_per_deg;
        if span_px >= target * 0.8 && span_px <= target * 2.5 {
            return z;
        }
    }
    10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lonlat_roundtrip_near_equator() {
        let (x, y) = lonlat_to_world_px(-95.0, 40.0, 10);
        let (lon, lat) = world_px_to_lonlat(x, y, 10);
        assert!((lon + 95.0).abs() < 0.01);
        assert!((lat - 40.0).abs() < 0.01);
    }

    #[test]
    fn tile_url_is_osm_standard() {
        let url = tile_url(10, 512, 341);
        assert!(url.contains("tile.openstreetmap.org"));
    }

    #[test]
    fn classify_osm_water_and_land_elevation() {
        let mut hm_img = RgbaImage::new(360, 180);
        for y in 0..180 {
            for x in 0..360 {
                hm_img.put_pixel(x, y, image::Rgba([0, 0, 140, 255]));
            }
        }
        hm_img.put_pixel(181, 90, image::Rgba([0, 0, 190, 255]));
        let hm = WorldHeightmap::from_image(hm_img);

        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, image::Rgba([170, 211, 223, 255]));
        img.put_pixel(1, 0, image::Rgba([242, 239, 233, 255]));
        let encoded = classify_osm_to_rgba_with_heightmap(&img, -1.0, 0.0, 1.0, 0.0, &hm);
        assert_eq!(encoded.get_pixel(0, 0).0[2], 106);
        assert!(
            encoded.get_pixel(1, 0).0[2] >= 179,
            "land should inherit heightmap mountain blue"
        );
    }

    #[test]
    fn classify_osm_land_uses_heightmap_blue() {
        let mut hm_img = RgbaImage::new(1, 1);
        hm_img.put_pixel(0, 0, image::Rgba([0, 0, 156, 255]));
        let hm = WorldHeightmap::from_image(hm_img);

        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, image::Rgba([242, 239, 233, 255]));
        let encoded =
            classify_osm_to_rgba_with_heightmap(&img, 0.0, 0.0, 0.0, 0.0, &hm);
        assert_eq!(encoded.get_pixel(0, 0).0[2], 156);
    }
}
