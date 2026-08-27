//! Stamp geographic bounds (SOWM v2 geo record) into existing map.bin files.
//!
//! Bbox resolution order: explicit `--bbox` (single map) → curated
//! [`KNOWN_MAP_BBOXES`] → `--calibrate` least-squares fit from the map's spawn
//! anchors against geo-entity centroids. The curated table is authoritative;
//! calibration is an assistant whose fits were reviewed and pasted there.

use sow_core::map_file::{self, GeoBounds, MapFile};
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct StampGeoArgs {
    pub maps_root: PathBuf,
    pub map: Option<String>,
    pub bbox: Option<String>,
    pub calibrate: bool,
    pub verify: bool,
    pub dry_run: bool,
    pub yes: bool,
}

/// Curated per-map bounds: (key, [min_lon, min_lat, max_lon, max_lat]).
/// Fitted by trimmed least squares from each map's spawn anchors (mean
/// residual ≤ ~1.3°). `max_lon > 180` marks maps crossing the antimeridian.
/// `pangaea` is fictional and intentionally absent.
const KNOWN_MAP_BBOXES: &[(&str, [f64; 4])] = &[
    ("africa", [-18.17, -36.78, 52.32, 38.07]),
    ("asia", [26.62, -8.50, 178.79, 88.41]),
    ("bajacalifornia", [-122.99, 19.05, -105.87, 35.29]),
    ("eastanglia", [-5.23, 50.41, 2.84, 52.92]),
    ("eastasia", [119.68, 26.19, 152.30, 52.46]),
    ("europe", [-25.47, 29.00, 47.75, 72.56]),
    ("indiansubcontinent", [60.81, -0.83, 96.61, 36.99]),
    ("mena", [-13.71, 9.76, 61.42, 43.03]),
    ("middleeast", [29.70, 12.02, 63.52, 40.08]),
    ("northamerica", [-169.40, 6.59, -17.95, 82.86]),
    ("oceania", [92.02, -54.69, 251.32, 23.54]),
    ("southamerica", [-92.95, -54.80, -35.02, 23.16]),
    ("southeastasia", [91.99, -10.70, 153.93, 25.31]),
    ("world", [-168.69, -78.80, 192.37, 82.78]),
];

/// Anchor-name aliases for calibration that don't (and shouldn't) exist in
/// the runtime geo database: name → (lat, lon).
const CALIBRATION_ALIASES: &[(&str, f64, f64)] = &[
    ("Türkiye", 39.9, 32.9),
    ("Turkey", 39.9, 32.9),
    ("Libyan Arab Jamahiriya", 27.0, 17.0),
    ("Syrian Arab Republic", 35.0, 38.0),
    ("Islamic Republic of Iran", 32.5, 53.0),
    ("England", 52.3, -1.5),
    ("Scotland", 56.5, -4.0),
    ("Wales", 52.3, -3.7),
    ("Northern Ireland", 54.6, -6.7),
    ("United Kingdom", 54.0, -2.5),
    ("Alaska", 64.0, -152.0),
    ("Hawaii", 20.8, -156.3),
    ("California", 37.2, -119.3),
    ("Texas", 31.5, -99.3),
    ("Florida", 28.6, -82.5),
    ("New York", 42.9, -75.5),
    ("Quebec", 52.0, -72.0),
    ("Yukon", 63.6, -135.8),
    ("Nunavut", 66.0, -93.0),
    ("Hong Kong", 22.3, 114.2),
    ("Tibet", 31.5, 88.0),
    ("Sakhalin", 50.3, 142.8),
    ("Okinawa", 26.5, 127.9),
    ("Tokyo", 35.68, 139.69),
    ("Seoul", 37.57, 126.98),
    ("Sumatra", -0.5, 101.4),
    ("Java", -7.3, 110.0),
    ("Kalimantan", 0.0, 114.0),
    ("Sulawesi", -2.0, 120.5),
];

/// Landmark cities for `--verify`: (name, lat, lon).
const VERIFY_CITIES: &[(&str, f64, f64)] = &[
    ("London", 51.51, -0.13),
    ("Paris", 48.86, 2.35),
    ("Rome", 41.90, 12.50),
    ("Moscow", 55.76, 37.62),
    ("Istanbul", 41.01, 28.98),
    ("Cairo", 30.04, 31.24),
    ("Lagos", 6.52, 3.38),
    ("Nairobi", -1.29, 36.82),
    ("Cape Town", -33.92, 18.42),
    ("Riyadh", 24.63, 46.72),
    ("Delhi", 28.61, 77.21),
    ("Bangkok", 13.76, 100.50),
    ("Beijing", 39.90, 116.40),
    ("Tokyo City", 35.68, 139.69),
    ("Sydney", -33.87, 151.21),
    ("Auckland", -36.85, 174.76),
    ("Honolulu", 21.31, -157.86),
    ("New York City", 40.71, -74.01),
    ("Mexico City", 19.43, -99.13),
    ("Lima", -12.05, -77.04),
    ("Sao Paulo", -23.55, -46.63),
];

pub fn run(args: StampGeoArgs) -> Result<(), Box<dyn Error>> {
    let keys: Vec<String> = match &args.map {
        Some(key) => vec![key.clone()],
        None => {
            let mut keys = Vec::new();
            for entry in fs::read_dir(&args.maps_root)? {
                let entry = entry?;
                if entry.path().is_dir() {
                    keys.push(entry.file_name().to_string_lossy().to_string());
                }
            }
            keys.sort();
            keys
        }
    };
    if args.bbox.is_some() && args.map.is_none() {
        return Err("--bbox requires --map <key> (one bbox per map)".into());
    }

    let write = args.yes && !args.dry_run;
    if !write {
        println!("(report only — pass --yes to write files)");
    }

    let mut stamped = 0usize;
    for key in &keys {
        let dir = args.maps_root.join(key);
        let Some(payload) = read_map_payload(&dir) else {
            println!("{key}: no readable map.bin/map.bin.br, skipping");
            continue;
        };
        let mut map = match map_file::parse(&payload) {
            Ok(m) => m,
            Err(e) => {
                println!("{key}: parse failed ({e}), skipping");
                continue;
            }
        };

        let bounds = resolve_bounds(&args, key, &map);
        let Some(bounds) = bounds else {
            println!("{key}: no bbox available (fictional or uncalibrated), stays v1");
            continue;
        };

        // Files in the transitional inline-geo layout (version 2) must be
        // rewritten even when the bounds match: the current trailing-record
        // layout keeps version 1 so pre-geo parsers accept stamped maps.
        let outdated_layout = u16::from_le_bytes([payload[4], payload[5]]) != map_file::MAP_VERSION;
        if map.geo_bounds == Some(bounds) && !outdated_layout {
            println!("{key}: already stamped with identical bounds, up to date");
        } else {
            println!(
                "{key}: {} -> bbox ({:.2}, {:.2}, {:.2}, {:.2})",
                if map.geo_bounds.is_some() {
                    "restamp"
                } else {
                    "stamp v2"
                },
                bounds.min_lon(),
                bounds.min_lat(),
                bounds.max_lon(),
                bounds.max_lat()
            );
            map.geo_bounds = Some(bounds);
            if write {
                write_map(&dir, &map)?;
                stamped += 1;
            }
        }

        if args.verify {
            verify_map(key, &map);
        }
    }

    if write {
        println!("Stamped {stamped} map(s).");
    }
    Ok(())
}

fn resolve_bounds(args: &StampGeoArgs, key: &str, map: &MapFile) -> Option<GeoBounds> {
    if let Some(bbox) = &args.bbox {
        return Some(parse_bbox_arg(bbox).expect("--bbox must be min_lon,min_lat,max_lon,max_lat"));
    }
    if let Some((_, b)) = KNOWN_MAP_BBOXES.iter().find(|(k, _)| k == &key) {
        return Some(GeoBounds::from_degrees(b[0], b[1], b[2], b[3]));
    }
    if args.calibrate {
        return calibrate(key, map);
    }
    None
}

fn parse_bbox_arg(s: &str) -> Option<GeoBounds> {
    let parts: Vec<f64> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    if parts.len() != 4 {
        return None;
    }
    Some(GeoBounds::from_degrees(
        parts[0], parts[1], parts[2], parts[3],
    ))
}

fn reference_latlon(name: &str) -> Option<(f64, f64)> {
    if let Some((_, lat, lon)) = CALIBRATION_ALIASES.iter().find(|(n, _, _)| *n == name) {
        return Some((*lat, *lon));
    }
    sow_core::geo_entities::all()
        .find(|e| e.name == name)
        .map(|e| (e.lat as f64, e.lon as f64))
}

/// Least squares deg = a·px + b over (px, deg) pairs.
fn lsq(pairs: &[(f64, f64)]) -> Option<(f64, f64)> {
    let n = pairs.len() as f64;
    let sx: f64 = pairs.iter().map(|(p, _)| p).sum();
    let sy: f64 = pairs.iter().map(|(_, d)| d).sum();
    let sxx: f64 = pairs.iter().map(|(p, _)| p * p).sum();
    let sxy: f64 = pairs.iter().map(|(p, d)| p * d).sum();
    let denom = n * sxx - sx * sx;
    if denom.abs() < f64::EPSILON {
        return None;
    }
    let a = (n * sxy - sx * sy) / denom;
    Some((a, (sy - a * sx) / n))
}

struct Fit {
    bounds: GeoBounds,
    matches: usize,
    mean_residual: f64,
}

/// Fit a linear lon(x)/lat(y) transform from named anchors, trying both plain
/// and antimeridian-shifted longitudes and keeping the better fit, with one
/// outlier-trim pass (hand-placed anchors can be 15° off).
fn fit_from_anchors(map: &MapFile) -> Option<Fit> {
    let matched: Vec<(f64, f64, f64, f64)> = map
        .spawns
        .iter()
        .filter_map(|s| {
            reference_latlon(&s.name).map(|(lat, lon)| (s.x as f64, s.y as f64, lat, lon))
        })
        .collect();
    if matched.len() < 4 {
        return None;
    }
    let attempt = |shift: bool| -> Option<Fit> {
        let mut pts: Vec<(f64, f64, f64, f64)> = matched
            .iter()
            .map(|&(x, y, lat, lon)| {
                (
                    x,
                    y,
                    lat,
                    if shift && lon < 0.0 { lon + 360.0 } else { lon },
                )
            })
            .collect();
        for pass in 0..2 {
            let (a, b) = lsq(&pts
                .iter()
                .map(|&(x, _, _, lon)| (x, lon))
                .collect::<Vec<_>>())?;
            let (c, d) = lsq(&pts
                .iter()
                .map(|&(_, y, lat, _)| (y, lat))
                .collect::<Vec<_>>())?;
            let residuals: Vec<f64> = pts
                .iter()
                .map(|&(x, y, lat, lon)| (a * x + b - lon).abs() + (c * y + d - lat).abs())
                .collect();
            let mut sorted = residuals.clone();
            sorted.sort_by(|p, q| p.partial_cmp(q).unwrap());
            let median = sorted[sorted.len() / 2];
            if pass == 0 {
                let keep: Vec<_> = pts
                    .iter()
                    .zip(&residuals)
                    .filter(|&(_, &r)| r <= (2.0 * median).max(1.0))
                    .map(|(p, _)| *p)
                    .collect();
                if keep.len() >= 4 && keep.len() < pts.len() {
                    pts = keep;
                    continue;
                }
            }
            let mean = residuals.iter().sum::<f64>() / residuals.len() as f64;
            return Some(Fit {
                bounds: GeoBounds::from_degrees(
                    b,
                    c * map.height as f64 + d,
                    a * map.width as f64 + b,
                    d,
                ),
                matches: pts.len(),
                mean_residual: mean,
            });
        }
        None
    };
    let plain = attempt(false);
    let shifted = attempt(true);
    match (plain, shifted) {
        (Some(p), Some(s)) => Some(if s.mean_residual < p.mean_residual {
            s
        } else {
            p
        }),
        (p, s) => p.or(s),
    }
}

fn calibrate(key: &str, map: &MapFile) -> Option<GeoBounds> {
    let fit = fit_from_anchors(map)?;
    println!(
        "{key}: calibration fit n={} mean_residual={:.2}° bbox=({:.2}, {:.2}, {:.2}, {:.2})",
        fit.matches,
        fit.mean_residual,
        fit.bounds.min_lon(),
        fit.bounds.min_lat(),
        fit.bounds.max_lon(),
        fit.bounds.max_lat()
    );
    if fit.matches >= 6 && fit.mean_residual < 3.0 {
        Some(fit.bounds)
    } else {
        println!(
            "{key}: fit too weak to auto-accept; review and add to KNOWN_MAP_BBOXES or pass --bbox"
        );
        None
    }
}

fn read_map_payload(dir: &Path) -> Option<Vec<u8>> {
    let bin = dir.join("map.bin");
    if let Ok(bytes) = fs::read(&bin) {
        return Some(bytes);
    }
    let br = dir.join("map.bin.br");
    let bytes = fs::read(&br).ok()?;
    map_file::decompress_map_payload(&bytes).ok()
}

fn brotli_compress(input: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut writer = brotli::CompressorWriter::new(&mut out, 4096, 11, 22);
    writer.write_all(input)?;
    writer.flush()?;
    drop(writer);
    Ok(out)
}

fn write_map(dir: &Path, map: &MapFile) -> Result<(), Box<dyn Error>> {
    let encoded = map_file::encode(map);
    fs::write(dir.join("map.bin"), &encoded)?;
    fs::write(dir.join("map.bin.br"), brotli_compress(&encoded)?)?;
    Ok(())
}

fn verify_map(key: &str, map: &MapFile) {
    let Some(bounds) = map.geo_bounds else {
        return;
    };
    println!("  verify {key}:");
    for (name, lat, lon) in VERIFY_CITIES {
        if let Some((x, y)) = bounds.project(*lat, *lon, map.width, map.height) {
            let tile = map.terrain[(y * map.width + x) as usize];
            let land = tile & 0x80 != 0;
            println!(
                "    {name}: tile ({x}, {y}) {}",
                if land { "land" } else { "water" }
            );
        }
    }
}
