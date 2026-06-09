use image::imageops::FilterType;
use image::{ImageBuffer, ImageEncoder, Rgba, RgbaImage};
use std::fs;
use std::path::{Path, PathBuf};

const CELL_PX: u32 = 64;
const TWEMOJI_BASE: &str = "https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/72x72";

pub struct PackEmojiAtlasArgs {
    pub repo_root: PathBuf,
    pub required: PathBuf,
    pub out_atlas: PathBuf,
    pub out_manifest: PathBuf,
}

pub fn pack(args: PackEmojiAtlasArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = &args.repo_root;
    let required = load_required(&args.required)?;
    let client = reqwest::blocking::Client::new();
    let mut entries: Vec<(String, RgbaImage)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    for emoji in &required {
        match fetch_twemoji(&client, emoji) {
            Ok(img) => entries.push((emoji.to_string(), img)),
            Err(e) => missing.push(format!("{emoji}: {e}")),
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

fn fetch_twemoji(
    client: &reqwest::blocking::Client,
    emoji: &str,
) -> Result<RgbaImage, Box<dyn std::error::Error + Send + Sync>> {
    for name in twemoji_filenames(emoji) {
        let url = format!("{TWEMOJI_BASE}/{name}.png");
        if let Ok(resp) = client.get(&url).send() {
            if resp.status().is_success() {
                let bytes = resp.bytes()?;
                let img = image::load_from_memory(&bytes)?.to_rgba8();
                return Ok(downscale_cell(&img));
            }
        }
    }
    Err("twemoji CDN miss".into())
}

fn downscale_cell(img: &RgbaImage) -> RgbaImage {
    if img.width() == CELL_PX && img.height() == CELL_PX {
        return img.clone();
    }
    image::imageops::resize(img, CELL_PX, CELL_PX, FilterType::Lanczos3)
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
        let escaped = emoji.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!(
            "        \"{escaped}\" => Some(AtlasRect {{ x: {x}, y: {y}, w: {w}, h: {h} }}),\n"
        ));
    }
    out.push_str("        _ => None,\n    }\n}\n");
    fs::write(path, out)?;
    Ok(())
}
