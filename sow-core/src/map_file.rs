//! Versioned `map.bin` and `catalog.bin` formats (no JSON).

pub const MAP_MAGIC: &[u8; 4] = b"SOWM";
pub const MAP_VERSION: u16 = 1;
/// Transitional inline-geo layout (record between spawns and terrain);
/// still parsed, never written. Current files append the geo record AFTER
/// the terrain instead, staying version 1 so pre-geo parsers (deployed
/// servers, cached wasm clients) read them unchanged and ignore the tail.
pub const MAP_VERSION_INLINE_GEO: u16 = 2;

/// Geo record tags: byte introducing the record.
const GEO_TAG_NONE: u8 = 0;
const GEO_TAG_EQUIRECT: u8 = 1;

pub const CATALOG_MAGIC: &[u8; 4] = b"SOWC";
pub const CATALOG_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MapSpawn {
    pub name: String,
    pub flag: String,
    pub x: u32,
    pub y: u32,
}

/// Geographic bounding box of a real-world map, equirectangular projection.
/// Degrees stored as fixed-point micro-degrees (E6) for exact roundtrip and `Eq`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GeoBounds {
    pub min_lon_e6: i32,
    pub min_lat_e6: i32,
    pub max_lon_e6: i32,
    pub max_lat_e6: i32,
}

impl GeoBounds {
    pub fn from_degrees(min_lon: f64, min_lat: f64, max_lon: f64, max_lat: f64) -> Self {
        Self {
            min_lon_e6: (min_lon * 1e6).round() as i32,
            min_lat_e6: (min_lat * 1e6).round() as i32,
            max_lon_e6: (max_lon * 1e6).round() as i32,
            max_lat_e6: (max_lat * 1e6).round() as i32,
        }
    }

    pub fn min_lon(&self) -> f64 {
        self.min_lon_e6 as f64 / 1e6
    }
    pub fn min_lat(&self) -> f64 {
        self.min_lat_e6 as f64 / 1e6
    }
    pub fn max_lon(&self) -> f64 {
        self.max_lon_e6 as f64 / 1e6
    }
    pub fn max_lat(&self) -> f64 {
        self.max_lat_e6 as f64 / 1e6
    }

    /// Pacific-centered maps cross the antimeridian and store `max_lon > 180`;
    /// input longitudes stay in [-180, 180] and get shifted up into the box.
    fn normalize_lon(&self, lon: f64) -> f64 {
        if self.max_lon_e6 > 180_000_000 && lon < self.min_lon() {
            lon + 360.0
        } else {
            lon
        }
    }

    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        let lon = self.normalize_lon(lon);
        lat >= self.min_lat()
            && lat <= self.max_lat()
            && lon >= self.min_lon()
            && lon <= self.max_lon()
    }

    /// lat/lon → tile coordinate. Equirectangular (linear lon→x, lat→y),
    /// mirroring the map generators' projection. Returns `None` outside the
    /// bounds or on a degenerate bbox. f64 basic ops only: bit-identical
    /// across native and wasm targets (lockstep-safe).
    pub fn project(&self, lat: f64, lon: f64, width: u32, height: u32) -> Option<(u32, u32)> {
        let lon_span = self.max_lon() - self.min_lon();
        let lat_span = self.max_lat() - self.min_lat();
        if lon_span <= 0.0 || lat_span <= 0.0 || !self.contains(lat, lon) {
            return None;
        }
        let lon = self.normalize_lon(lon);
        let x = ((lon - self.min_lon()) / lon_span * width as f64) as u32;
        let y = ((self.max_lat() - lat) / lat_span * height as f64) as u32;
        Some((
            x.min(width.saturating_sub(1)),
            y.min(height.saturating_sub(1)),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapHeader {
    pub display_name: String,
    pub width: u32,
    pub height: u32,
    pub num_land_tiles: u32,
    pub spawn_count: usize,
    pub geo_bounds: Option<GeoBounds>,
    pub header_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapFile {
    pub display_name: String,
    pub width: u32,
    pub height: u32,
    pub num_land_tiles: u32,
    pub spawns: Vec<MapSpawn>,
    pub geo_bounds: Option<GeoBounds>,
    pub terrain: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapCatalogEntry {
    pub key: String,
    pub display_name: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapCatalog {
    pub entries: Vec<MapCatalogEntry>,
}

#[derive(Debug)]
pub enum MapFileError {
    TooShort,
    BadMagic,
    UnsupportedVersion(u16),
    BadGeoTag(u8),
    InvalidUtf8,
    TerrainLengthMismatch { expected: usize, got: usize },
}

impl std::fmt::Display for MapFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "map file too short"),
            Self::BadMagic => write!(f, "invalid map magic (expected SOWM)"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported map version {v}"),
            Self::BadGeoTag(t) => write!(f, "unsupported geo record tag {t}"),
            Self::InvalidUtf8 => write!(f, "invalid utf-8 in map file"),
            Self::TerrainLengthMismatch { expected, got } => {
                write!(f, "terrain length mismatch: expected {expected}, got {got}")
            }
        }
    }
}

impl std::error::Error for MapFileError {}

fn read_u16(data: &[u8], off: &mut usize) -> Option<u16> {
    let bytes = data.get(*off..*off + 2)?;
    *off += 2;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], off: &mut usize) -> Option<u32> {
    let bytes = data.get(*off..*off + 4)?;
    *off += 4;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_i32(data: &[u8], off: &mut usize) -> Option<i32> {
    let bytes = data.get(*off..*off + 4)?;
    *off += 4;
    Some(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_u8(data: &[u8], off: &mut usize) -> Option<u8> {
    let b = *data.get(*off)?;
    *off += 1;
    Some(b)
}

/// Read the v2+ geo record (tag byte + optional bbox).
fn read_geo_record(data: &[u8], off: &mut usize) -> Result<Option<GeoBounds>, MapFileError> {
    let tag = read_u8(data, off).ok_or(MapFileError::TooShort)?;
    match tag {
        GEO_TAG_NONE => Ok(None),
        GEO_TAG_EQUIRECT => {
            let min_lon_e6 = read_i32(data, off).ok_or(MapFileError::TooShort)?;
            let min_lat_e6 = read_i32(data, off).ok_or(MapFileError::TooShort)?;
            let max_lon_e6 = read_i32(data, off).ok_or(MapFileError::TooShort)?;
            let max_lat_e6 = read_i32(data, off).ok_or(MapFileError::TooShort)?;
            Ok(Some(GeoBounds {
                min_lon_e6,
                min_lat_e6,
                max_lon_e6,
                max_lat_e6,
            }))
        }
        other => Err(MapFileError::BadGeoTag(other)),
    }
}

fn read_string(data: &[u8], off: &mut usize) -> Result<String, MapFileError> {
    let len = read_u16(data, off).ok_or(MapFileError::TooShort)? as usize;
    let slice = data.get(*off..*off + len).ok_or(MapFileError::TooShort)?;
    *off += len;
    std::str::from_utf8(slice)
        .map(|s| s.to_owned())
        .map_err(|_| MapFileError::InvalidUtf8)
}

fn write_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_i32(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_string(out: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    debug_assert!(bytes.len() <= u16::MAX as usize);
    write_u16(out, bytes.len() as u16);
    out.extend_from_slice(bytes);
}

/// Parse map header + spawn table; does not copy terrain.
pub fn parse_header(data: &[u8]) -> Result<MapHeader, MapFileError> {
    if data.len() < 20 {
        return Err(MapFileError::TooShort);
    }
    if data.get(0..4) != Some(MAP_MAGIC) {
        return Err(MapFileError::BadMagic);
    }
    let mut off = 4usize;
    let version = read_u16(data, &mut off).ok_or(MapFileError::TooShort)?;
    if version != MAP_VERSION && version != MAP_VERSION_INLINE_GEO {
        return Err(MapFileError::UnsupportedVersion(version));
    }
    off += 2; // reserved
    let width = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
    let height = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
    let num_land_tiles = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
    let display_name = read_string(data, &mut off)?;
    let spawn_count = read_u16(data, &mut off).ok_or(MapFileError::TooShort)? as usize;
    for _ in 0..spawn_count {
        let _name = read_string(data, &mut off)?;
        let _flag = read_string(data, &mut off)?;
        let _ = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
        let _ = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
    }
    let mut geo_bounds = None;
    if version == MAP_VERSION_INLINE_GEO {
        // Transitional layout: record sits between spawns and terrain.
        geo_bounds = read_geo_record(data, &mut off)?;
    }
    let header_bytes = off;
    if version == MAP_VERSION {
        // Current layout: record trails the terrain. Best-effort here — a
        // caller may pass a header-only prefix, and pre-geo files simply
        // end at the terrain.
        if let Some(terrain_len) = (width as usize).checked_mul(height as usize) {
            let mut tail = header_bytes.saturating_add(terrain_len);
            if data.len() > tail {
                geo_bounds = read_geo_record(data, &mut tail)?;
            }
        }
    }
    Ok(MapHeader {
        display_name,
        width,
        height,
        num_land_tiles,
        spawn_count,
        geo_bounds,
        header_bytes,
    })
}

/// Full map parse (header + spawns + terrain).
pub fn parse(data: &[u8]) -> Result<MapFile, MapFileError> {
    let header = parse_header(data)?;
    let terrain_len = (header.width as usize)
        .checked_mul(header.height as usize)
        .ok_or(MapFileError::TooShort)?;
    let terrain = data
        .get(header.header_bytes..header.header_bytes + terrain_len)
        .ok_or(MapFileError::TerrainLengthMismatch {
            expected: terrain_len,
            got: data.len().saturating_sub(header.header_bytes),
        })?
        .to_vec();
    if terrain.len() != terrain_len {
        return Err(MapFileError::TerrainLengthMismatch {
            expected: terrain_len,
            got: terrain.len(),
        });
    }

    let mut off = 4usize;
    let _version = read_u16(data, &mut off).ok_or(MapFileError::TooShort)?;
    off += 2;
    let _width = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
    let _height = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
    let num_land_tiles = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
    let display_name = read_string(data, &mut off)?;
    let spawn_count = read_u16(data, &mut off).ok_or(MapFileError::TooShort)? as usize;
    let mut spawns = Vec::with_capacity(spawn_count);
    for _ in 0..spawn_count {
        let name = read_string(data, &mut off)?;
        let flag = read_string(data, &mut off)?;
        let x = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
        let y = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
        spawns.push(MapSpawn { name, flag, x, y });
    }

    Ok(MapFile {
        display_name,
        width: header.width,
        height: header.height,
        num_land_tiles,
        spawns,
        geo_bounds: header.geo_bounds,
        terrain,
    })
}

pub fn encode(map: &MapFile) -> Vec<u8> {
    let terrain_len = (map.width as usize) * (map.height as usize);
    debug_assert_eq!(map.terrain.len(), terrain_len);

    // Always version 1: the optional geo record trails the terrain, where
    // pre-geo parsers never look. Unstamped maps stay byte-identical to the
    // original format.
    let mut out = Vec::with_capacity(32 + map.terrain.len());
    out.extend_from_slice(MAP_MAGIC);
    write_u16(&mut out, MAP_VERSION);
    write_u16(&mut out, 0);
    write_u32(&mut out, map.width);
    write_u32(&mut out, map.height);
    write_u32(&mut out, map.num_land_tiles);
    write_string(&mut out, &map.display_name);
    write_u16(&mut out, map.spawns.len() as u16);
    for s in &map.spawns {
        write_string(&mut out, &s.name);
        write_string(&mut out, &s.flag);
        write_u32(&mut out, s.x);
        write_u32(&mut out, s.y);
    }
    out.extend_from_slice(&map.terrain);
    if let Some(b) = map.geo_bounds {
        out.push(GEO_TAG_EQUIRECT);
        write_i32(&mut out, b.min_lon_e6);
        write_i32(&mut out, b.min_lat_e6);
        write_i32(&mut out, b.max_lon_e6);
        write_i32(&mut out, b.max_lat_e6);
    }
    out
}

pub fn encode_catalog(catalog: &MapCatalog) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(CATALOG_MAGIC);
    write_u16(&mut out, CATALOG_VERSION);
    write_u16(&mut out, 0);
    write_u32(&mut out, catalog.entries.len() as u32);
    for e in &catalog.entries {
        write_string(&mut out, &e.key);
        write_string(&mut out, &e.display_name);
        write_u32(&mut out, e.width);
        write_u32(&mut out, e.height);
    }
    out
}

pub fn parse_catalog(data: &[u8]) -> Result<MapCatalog, MapFileError> {
    if data.len() < 12 {
        return Err(MapFileError::TooShort);
    }
    if data.get(0..4) != Some(CATALOG_MAGIC) {
        return Err(MapFileError::BadMagic);
    }
    let mut off = 4usize;
    let version = read_u16(data, &mut off).ok_or(MapFileError::TooShort)?;
    if version != CATALOG_VERSION {
        return Err(MapFileError::UnsupportedVersion(version));
    }
    off += 2;
    let count = read_u32(data, &mut off).ok_or(MapFileError::TooShort)? as usize;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let key = read_string(data, &mut off)?;
        let display_name = read_string(data, &mut off)?;
        let width = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
        let height = read_u32(data, &mut off).ok_or(MapFileError::TooShort)?;
        entries.push(MapCatalogEntry {
            key,
            display_name,
            width,
            height,
        });
    }
    Ok(MapCatalog { entries })
}

/// Build catalog from map folder keys and parsed headers.
pub fn catalog_from_headers(items: impl IntoIterator<Item = (String, MapHeader)>) -> MapCatalog {
    let mut entries: Vec<MapCatalogEntry> = items
        .into_iter()
        .map(|(key, h)| MapCatalogEntry {
            key,
            display_name: h.display_name,
            width: h.width,
            height: h.height,
        })
        .collect();
    entries.sort_by_key(|a| a.display_name.to_lowercase());
    MapCatalog { entries }
}

/// Decompress a `map.bin.br` payload (no-op if already decompressed SOWM).
pub fn decompress_map_payload(bytes: &[u8]) -> Result<Vec<u8>, MapFileError> {
    if bytes.len() >= 4 && &bytes[0..4] == MAP_MAGIC {
        return Ok(bytes.to_vec());
    }
    let mut out = Vec::new();
    let mut decoder = brotli::Decompressor::new(bytes, 4096);
    std::io::Read::read_to_end(&mut decoder, &mut out).map_err(|_| MapFileError::TooShort)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_map(geo_bounds: Option<GeoBounds>) -> MapFile {
        MapFile {
            display_name: "North America".to_string(),
            width: 4,
            height: 4,
            num_land_tiles: 10,
            spawns: vec![MapSpawn {
                name: "Rome".to_string(),
                flag: "it".to_string(),
                x: 1,
                y: 2,
            }],
            geo_bounds,
            terrain: vec![0u8; 16],
        }
    }

    #[test]
    fn roundtrip_map_file() {
        let map = sample_map(None);
        let bytes = encode(&map);
        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.display_name, "North America");
        assert_eq!(parsed.spawns.len(), 1);
        assert_eq!(parsed.terrain.len(), 16);
        assert_eq!(parsed.geo_bounds, None);
    }

    #[test]
    fn no_bounds_encodes_as_v1() {
        let bytes = encode(&sample_map(None));
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 1);
    }

    #[test]
    fn roundtrip_with_trailing_bounds() {
        let bounds = GeoBounds::from_degrees(-30.5, 10.25, 60.0, 72.125);
        let map = sample_map(Some(bounds));
        let bytes = encode(&map);
        // Stays version 1: pre-geo parsers must accept stamped maps.
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 1);
        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.geo_bounds, Some(bounds));
        assert_eq!(parsed.terrain.len(), 16);
        assert_eq!(parsed.spawns, map.spawns);
        let header = parse_header(&bytes).unwrap();
        assert_eq!(header.geo_bounds, Some(bounds));
        // Terrain sits at header_bytes; the geo record trails it.
        let terrain_end = header.header_bytes + map.terrain.len();
        assert_eq!(&bytes[header.header_bytes..terrain_end], &map.terrain[..]);
        assert_eq!(bytes.len(), terrain_end + 17);
    }

    /// A stamped file must parse under the ORIGINAL v1 rules (what deployed
    /// servers and cached wasm clients run): version==1, terrain sliced by
    /// exact length from the spawn-table end, trailing bytes ignored.
    #[test]
    fn stamped_file_readable_by_pre_geo_parser() {
        let map = sample_map(Some(GeoBounds::from_degrees(0.0, 0.0, 10.0, 10.0)));
        let bytes = encode(&map);
        // Header prefix only (what a pre-geo parse_header consumed) must be
        // byte-identical to the unstamped encoding's prefix.
        let unstamped = encode(&sample_map(None));
        let header = parse_header(&unstamped).unwrap();
        assert_eq!(
            bytes[..header.header_bytes],
            unstamped[..header.header_bytes]
        );
        // Old parse: exact-length terrain slice succeeds.
        let terrain = bytes
            .get(header.header_bytes..header.header_bytes + map.terrain.len())
            .unwrap();
        assert_eq!(terrain, &map.terrain[..]);
    }

    /// Transitional inline layout (version 2, record between spawns and
    /// terrain) is still parsed for files stamped before the format moved
    /// the record behind the terrain.
    #[test]
    fn parses_transitional_inline_geo_layout() {
        let bounds = GeoBounds::from_degrees(-5.0, 40.0, 25.0, 60.0);
        let map = sample_map(Some(bounds));
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAP_MAGIC);
        write_u16(&mut bytes, MAP_VERSION_INLINE_GEO);
        write_u16(&mut bytes, 0);
        write_u32(&mut bytes, map.width);
        write_u32(&mut bytes, map.height);
        write_u32(&mut bytes, map.num_land_tiles);
        write_string(&mut bytes, &map.display_name);
        write_u16(&mut bytes, map.spawns.len() as u16);
        for s in &map.spawns {
            write_string(&mut bytes, &s.name);
            write_string(&mut bytes, &s.flag);
            write_u32(&mut bytes, s.x);
            write_u32(&mut bytes, s.y);
        }
        bytes.push(1); // GEO_TAG_EQUIRECT inline
        write_i32(&mut bytes, bounds.min_lon_e6);
        write_i32(&mut bytes, bounds.min_lat_e6);
        write_i32(&mut bytes, bounds.max_lon_e6);
        write_i32(&mut bytes, bounds.max_lat_e6);
        bytes.extend_from_slice(&map.terrain);
        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.geo_bounds, Some(bounds));
        assert_eq!(parsed.terrain, map.terrain);
    }

    #[test]
    fn truncated_geo_record_is_too_short() {
        let bounds = GeoBounds::from_degrees(0.0, 0.0, 10.0, 10.0);
        let bytes = encode(&sample_map(Some(bounds)));
        // Cut inside the trailing geo record (after terrain).
        let cut = bytes.len() - 8;
        assert!(matches!(parse(&bytes[..cut]), Err(MapFileError::TooShort)));
    }

    #[test]
    fn future_version_rejected() {
        let mut bytes = encode(&sample_map(None));
        bytes[4..6].copy_from_slice(&3u16.to_le_bytes());
        assert!(matches!(
            parse(&bytes),
            Err(MapFileError::UnsupportedVersion(3))
        ));
    }

    #[test]
    fn bad_geo_tag_rejected() {
        let bounds = GeoBounds::from_degrees(0.0, 0.0, 10.0, 10.0);
        let map = sample_map(Some(bounds));
        let mut bytes = encode(&map);
        let tag_off = bytes.len() - 17;
        assert_eq!(bytes[tag_off], 1);
        bytes[tag_off] = 7;
        assert!(matches!(parse(&bytes), Err(MapFileError::BadGeoTag(7))));
    }

    #[test]
    fn project_maps_bounds_to_tiles() {
        // World-style full bbox on a 1000x800 grid.
        let b = GeoBounds::from_degrees(-180.0, -90.0, 180.0, 90.0);
        assert_eq!(b.project(90.0, -180.0, 1000, 800), Some((0, 0)));
        assert_eq!(b.project(-90.0, 180.0, 1000, 800), Some((999, 799)));
        // Equator/meridian center.
        assert_eq!(b.project(0.0, 0.0, 1000, 800), Some((500, 400)));
        // Matches poi_extractor formula: x=(lon-min_lon)*scale, y=(max_lat-lat)*scale.
        let e = GeoBounds::from_degrees(-10.0, 35.0, 40.0, 70.0);
        let (w, h) = (500u32, 350u32); // 10 px/degree
        let (x, y) = e.project(48.85, 2.35, w, h).unwrap(); // Paris
        assert_eq!(x, ((2.35 - -10.0) * 10.0) as u32);
        assert_eq!(y, ((70.0 - 48.85) * 10.0) as u32);
        // Outside → None.
        assert_eq!(e.project(40.7, -74.0, w, h), None); // New York not in Europe
    }

    #[test]
    fn project_handles_antimeridian_wrap() {
        // Pacific box from lon 90 to 210 (= -150): Hawaii at -155 is inside.
        let b = GeoBounds::from_degrees(90.0, -50.0, 210.0, 30.0);
        assert!(b.contains(21.3, -157.86)); // Honolulu
        assert!(b.contains(-36.85, 174.76)); // Auckland
        assert!(!b.contains(48.85, 2.35)); // Paris
        let (w, h) = (1200u32, 800u32);
        let (x_fiji, _) = b.project(-17.8, 178.0, w, h).unwrap();
        let (x_hawaii, _) = b.project(21.3, -157.86, w, h).unwrap();
        assert!(x_hawaii > x_fiji, "Hawaii must land east of Fiji");
    }

    #[test]
    fn roundtrip_catalog() {
        let cat = MapCatalog {
            entries: vec![MapCatalogEntry {
                key: "world".to_string(),
                display_name: "North America".to_string(),
                width: 100,
                height: 100,
            }],
        };
        let bytes = encode_catalog(&cat);
        let parsed = parse_catalog(&bytes).unwrap();
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].key, "world");
    }
}
