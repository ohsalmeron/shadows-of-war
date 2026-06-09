use image::imageops::FilterType;
use image::{ImageBuffer, ImageEncoder, Rgba, RgbaImage};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const CELL_PX: u32 = 32;
const PTWMC_GLYPH_PX: u32 = 9;
const TWEMOJI_BASE: &str = "https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/72x72";
const PTWMC_PIN: &str = "vendor/ptwmc/assets/twemoji/textures/font/emoji.png";
const PTWMC_FONT_JSON: &str = "vendor/ptwmc/assets/minecraft/font/default.json";

pub struct PackEmojiAtlasArgs {
    pub repo_root: PathBuf,
    pub required: PathBuf,
    pub out_atlas: PathBuf,
    pub out_manifest: PathBuf,
}

pub fn pack(args: PackEmojiAtlasArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let required = load_required(&args.required)?;
    let sheet_path = args.repo_root.join(PTWMC_PIN);
    let font_json_path = args.repo_root.join(PTWMC_FONT_JSON);
    if !sheet_path.is_file() {
        return Err(format!(
            "missing {PTWMC_PIN} — extract PixelTwemojiMC v8.0 into vendor/ptwmc (see assets/SOURCES.toml)"
        )
        .into());
    }

    let grid = parse_mc_font_grid(&font_json_path)?;
    let sheet = image::open(&sheet_path)?.to_rgba8();
    let cols = sheet.width() / PTWMC_GLYPH_PX;

    let client = reqwest::blocking::Client::new();
    let mut entries: Vec<(String, RgbaImage)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    for emoji in &required {
        if let Some(img) = extract_ptwmc(&sheet, cols, &grid, emoji) {
            entries.push((emoji.to_string(), upscale_nearest(&img, CELL_PX)));
            continue;
        }
        match fetch_twemoji_fallback(&client, emoji) {
            Ok(img) => entries.push((emoji.to_string(), img)),
            Err(e) => {
                missing.push(format!("{emoji}: {e}"));
            }
        }
    }

    if !missing.is_empty() {
        return Err(format!("missing emoji glyphs:\n{}", missing.join("\n")).into());
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let (atlas, rects) = pack_grid(&entries, CELL_PX)?;
    write_webp(&atlas, &args.out_atlas)?;
    write_manifest_rs(&rects, atlas.width(), atlas.height(), &args.out_manifest)?;
    println!(
        "Packed {} glyphs → {} ({}x{})",
        entries.len(),
        args.out_atlas.display(),
        atlas.width(),
        atlas.height()
    );
    Ok(())
}

fn load_required(path: &Path) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let raw = fs::read_to_string(path)?;
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !out.contains(&line.to_string()) {
            out.push(line.to_string());
        }
    }
    Ok(out)
}

fn parse_mc_font_grid(
    font_json: &Path,
) -> Result<HashMap<char, (u32, u32)>, Box<dyn std::error::Error + Send + Sync>> {
    let data: Value = serde_json::from_str(&fs::read_to_string(font_json)?)?;
    let providers = data["providers"]
        .as_array()
        .ok_or("font providers array")?;
    let mut map = HashMap::new();
    let mut row = 0u32;
    for provider in providers {
        if provider["type"].as_str() != Some("bitmap") {
            continue;
        }
        let chars = provider["chars"]
            .as_array()
            .ok_or("bitmap chars")?;
        for row_str in chars {
            let s = row_str.as_str().ok_or("char row string")?;
            for (col, ch) in s.chars().enumerate() {
                map.insert(ch, (col as u32, row));
            }
            row += 1;
        }
    }
    Ok(map)
}

fn extract_ptwmc(
    sheet: &RgbaImage,
    cols: u32,
    grid: &HashMap<char, (u32, u32)>,
    emoji: &str,
) -> Option<RgbaImage> {
    let chars: Vec<char> = emoji.chars().collect();
    if chars.len() == 1 {
        if let Some(&(col, row)) = grid.get(&chars[0]) {
            return Some(crop_glyph(sheet, cols, col, row));
        }
        return None;
    }
    // ZWJ / multi-codepoint: use first base character for pixel sheet lookup.
    if let Some(&(col, row)) = grid.get(&chars[0]) {
        return Some(crop_glyph(sheet, cols, col, row));
    }
    None
}

fn crop_glyph(sheet: &RgbaImage, _cols: u32, col: u32, row: u32) -> RgbaImage {
    let x = col * PTWMC_GLYPH_PX;
    let y = row * PTWMC_GLYPH_PX;
    image::imageops::crop_imm(sheet, x, y, PTWMC_GLYPH_PX, PTWMC_GLYPH_PX).to_image()
}

fn upscale_nearest(img: &RgbaImage, size: u32) -> RgbaImage {
    image::imageops::resize(img, size, size, FilterType::Nearest)
}

fn fetch_twemoji_fallback(
    client: &reqwest::blocking::Client,
    emoji: &str,
) -> Result<RgbaImage, Box<dyn std::error::Error + Send + Sync>> {
    for name in twemoji_filenames(emoji) {
        let url = format!("{TWEMOJI_BASE}/{name}.png");
        if let Ok(resp) = client.get(&url).send() {
            if resp.status().is_success() {
                let bytes = resp.bytes()?;
                let img = image::load_from_memory(&bytes)?.to_rgba8();
                return Ok(upscale_nearest(&img, CELL_PX));
            }
        }
    }
    Err("twemoji CDN miss".into())
}

fn twemoji_filenames(emoji: &str) -> Vec<String> {
    let cps: Vec<u32> = emoji.chars().map(|c| c as u32).collect();
    let mut names = Vec::new();
    if cps.len() == 1 {
        names.push(format!("{:x}", cps[0]));
        return names;
    }
    names.push(
        cps.iter()
            .map(|cp| format!("{cp:x}"))
            .collect::<Vec<_>>()
            .join("-"),
    );
    let no_fe0f: Vec<u32> = cps.iter().copied().filter(|&cp| cp != 0xfe0f).collect();
    if no_fe0f.len() != cps.len() {
        names.push(
            no_fe0f
                .iter()
                .map(|cp| format!("{cp:x}"))
                .collect::<Vec<_>>()
                .join("-"),
        );
    }
    if let Some(base) = cps.first() {
        names.push(format!("{base:x}"));
    }
    names
}

fn pack_grid(
    entries: &[(String, RgbaImage)],
    cell: u32,
) -> Result<(RgbaImage, Vec<(String, u32, u32, u32, u32)>), Box<dyn std::error::Error + Send + Sync>>
{
    let n = entries.len() as u32;
    let cols = (n as f32).sqrt().ceil() as u32;
    let rows = n.div_ceil(cols);
    let w = cols * cell;
    let h = rows * cell;
    let mut atlas: RgbaImage = ImageBuffer::from_pixel(w, h, Rgba([0, 0, 0, 0]));
    let mut rects = Vec::new();
    for (i, (emoji, img)) in entries.iter().enumerate() {
        let col = (i as u32) % cols;
        let row = (i as u32) / cols;
        let x = col * cell;
        let y = row * cell;
        image::imageops::overlay(&mut atlas, img, x.into(), y.into());
        rects.push((emoji.clone(), x, y, cell, cell));
    }
    Ok((atlas, rects))
}

fn write_webp(atlas: &RgbaImage, path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let encoder = image::codecs::webp::WebPEncoder::new_lossless(fs::File::create(path)?);
    encoder.write_image(
        atlas.as_raw(),
        atlas.width(),
        atlas.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(())
}

fn write_manifest_rs(
    rects: &[(String, u32, u32, u32, u32)],
    atlas_w: u32,
    atlas_h: u32,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = String::from("// @generated by sow-tools pack-emoji-atlas — do not edit\n\n");
    out.push_str(&format!("pub const ATLAS_WIDTH: u32 = {atlas_w};\n"));
    out.push_str(&format!("pub const ATLAS_HEIGHT: u32 = {atlas_h};\n\n"));
    out.push_str("pub struct AtlasRect {\n    pub x: u32,\n    pub y: u32,\n    pub w: u32,\n    pub h: u32,\n}\n\n");
    out.push_str("pub fn lookup(emoji: &str) -> Option<AtlasRect> {\n    match emoji {\n");
    for (emoji, x, y, w, h) in rects {
        let escaped = emoji
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        out.push_str(&format!(
            "        \"{escaped}\" => Some(AtlasRect {{ x: {x}, y: {y}, w: {w}, h: {h} }}),\n"
        ));
    }
    out.push_str("        _ => None,\n    }\n}\n");
    fs::write(path, out)?;
    Ok(())
}
