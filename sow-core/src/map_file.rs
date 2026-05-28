//! Versioned `map.bin` and `catalog.bin` formats (no JSON).

pub const MAP_MAGIC: &[u8; 4] = b"SOWM";
pub const MAP_VERSION: u16 = 1;

pub const CATALOG_MAGIC: &[u8; 4] = b"SOWC";
pub const CATALOG_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapSpawn {
    pub name: String,
    pub flag: String,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapHeader {
    pub display_name: String,
    pub width: u32,
    pub height: u32,
    pub num_land_tiles: u32,
    pub spawn_count: usize,
    pub header_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapFile {
    pub display_name: String,
    pub width: u32,
    pub height: u32,
    pub num_land_tiles: u32,
    pub spawns: Vec<MapSpawn>,
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
    InvalidUtf8,
    TerrainLengthMismatch { expected: usize, got: usize },
}

impl std::fmt::Display for MapFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "map file too short"),
            Self::BadMagic => write!(f, "invalid map magic (expected SOWM)"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported map version {v}"),
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
    if version != MAP_VERSION {
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
    let header_bytes = off;
    Ok(MapHeader {
        display_name,
        width,
        height,
        num_land_tiles,
        spawn_count,
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
        terrain,
    })
}

pub fn encode(map: &MapFile) -> Vec<u8> {
    let terrain_len = (map.width as usize) * (map.height as usize);
    debug_assert_eq!(map.terrain.len(), terrain_len);

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
pub fn catalog_from_headers(
    items: impl IntoIterator<Item = (String, MapHeader)>,
) -> MapCatalog {
    let mut entries: Vec<MapCatalogEntry> = items
        .into_iter()
        .map(|(key, h)| MapCatalogEntry {
            key,
            display_name: h.display_name,
            width: h.width,
            height: h.height,
        })
        .collect();
    entries.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
    MapCatalog { entries }
}

/// Decompress a `map.bin.br` payload (no-op if already decompressed SOWM).
pub fn decompress_map_payload(bytes: &[u8]) -> Result<Vec<u8>, MapFileError> {
    if bytes.len() >= 4 && &bytes[0..4] == MAP_MAGIC {
        return Ok(bytes.to_vec());
    }
    let mut out = Vec::new();
    let mut decoder = brotli::Decompressor::new(bytes, 4096);
    std::io::Read::read_to_end(&mut decoder, &mut out)
        .map_err(|_| MapFileError::TooShort)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_map_file() {
        let map = MapFile {
            display_name: "Tutorial".to_string(),
            width: 4,
            height: 4,
            num_land_tiles: 10,
            spawns: vec![MapSpawn {
                name: "Rome".to_string(),
                flag: "it".to_string(),
                x: 1,
                y: 2,
            }],
            terrain: vec![0u8; 16],
        };
        let bytes = encode(&map);
        let parsed = parse(&bytes).unwrap();
        assert_eq!(parsed.display_name, "Tutorial");
        assert_eq!(parsed.spawns.len(), 1);
        assert_eq!(parsed.terrain.len(), 16);
    }

    #[test]
    fn roundtrip_catalog() {
        let cat = MapCatalog {
            entries: vec![MapCatalogEntry {
                key: "tutorial".to_string(),
                display_name: "Tutorial".to_string(),
                width: 100,
                height: 100,
            }],
        };
        let bytes = encode_catalog(&cat);
        let parsed = parse_catalog(&bytes).unwrap();
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].key, "tutorial");
    }
}
