use image::imageops::FilterType;
use image::{ImageBuffer, ImageEncoder, Rgba, RgbaImage};
use std::fs;
use std::path::{Path, PathBuf};

const CELL_PX: u32 = 64;
const MOJI_BASE: &str = "https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/72x72";

pub struct PackEmojiAtlasArgs {
    pub repo_root: PathBuf,
    pub out_atlas: PathBuf,
    pub out_manifest: PathBuf,
}

pub fn pack(args: PackEmojiAtlasArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let required = scan_source_for_emojis(&args.repo_root)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new());
    let mut entries: Vec<(String, RgbaImage)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    for emoji in &required {
        if std::env::var("VERBOSE").is_ok() {
            println!("Fetching moji for emoji: {emoji}");
        }
        match fetch_moji(&client, emoji) {
            Ok(img) => entries.push((emoji.to_string(), img)),
            Err(e) => {
                // Silently skip CDN 404s (e.g. false positives from source scanning)
                if !e.to_string().contains("moji CDN miss") {
                    missing.push(format!("{emoji}: {e}"));
                }
            }
        }
    }

    if !missing.is_empty() {
        println!(
            "Warning: some non-404 fetch errors occurred:\n{}",
            missing.join("\n")
        );
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

fn scan_source_for_emojis(
    repo_root: &Path,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut out = Vec::new();
    let mut stack = vec![repo_root.to_path_buf()];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            if path != repo_root {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.starts_with('.')
                    || name == "target"
                    || name == "dist"
                    || name == "node_modules"
                {
                    continue;
                }
                if path.parent() == Some(repo_root) && !name.starts_with("sow-") {
                    continue;
                }
            }
            if std::env::var("VERBOSE").is_ok() {
                println!("Scanning directory: {}", path.display());
            }
            for entry in fs::read_dir(path)? {
                stack.push(entry?.path());
            }
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            if std::env::var("VERBOSE").is_ok() {
                println!("Scanning Rust file: {}", path.display());
            }
            if let Ok(content) = fs::read_to_string(&path) {
                let mut current_emoji = String::new();
                for c in content.chars() {
                    let cp = c as u32;

                    // Filter out box/line/geometric drawing characters (0x2500 - 0x257F etc.)
                    // and basic punctuation ranges that bleed into emoji ranges.
                    let is_line_drawing =
                        (0x2500..=0x257F).contains(&cp) || cp == 0x2500 || cp == 0x2550;

                    let is_emoji = !is_line_drawing
                        && ((0x203C..=0x3299).contains(&cp)
                            || (0x1F000..=0x1FAFF).contains(&cp)
                            || cp == 0xFE0F
                            || cp == 0x200D
                            || cp == 0x20E3);

                    if is_emoji {
                        current_emoji.push(c);
                    } else if !current_emoji.is_empty() {
                        let trimmed = current_emoji.trim();
                        // Ignore pure line/dashes/punctuation strings
                        if !trimmed.is_empty()
                            && !trimmed
                                .chars()
                                .all(|ch| ch == '─' || ch == '═' || ch == '━' || ch == '═')
                            && !out.contains(&current_emoji)
                        {
                            out.push(current_emoji.clone());
                        }
                        current_emoji.clear();
                    }
                }
                if !current_emoji.is_empty() {
                    let trimmed = current_emoji.trim();
                    if !trimmed.is_empty()
                        && !trimmed
                            .chars()
                            .all(|ch| ch == '─' || ch == '═' || ch == '━' || ch == '═')
                        && !out.contains(&current_emoji)
                    {
                        out.push(current_emoji);
                    }
                }
            }
        }
    }
    Ok(out)
}

fn fetch_moji(
    client: &reqwest::blocking::Client,
    emoji: &str,
) -> Result<RgbaImage, Box<dyn std::error::Error + Send + Sync>> {
    let cache_dir = Path::new("assets/emoji/cache");
    fs::create_dir_all(cache_dir)?;

    let filenames = moji_filenames(emoji);
    if filenames.is_empty() {
        return Err("No filenames generated for emoji".into());
    }

    // Try finding in cache first
    for name in &filenames {
        let cache_path = cache_dir.join(format!("{}.png", name));
        if cache_path.exists() {
            if let Ok(bytes) = fs::read(&cache_path) {
                if let Ok(img) = image::load_from_memory(&bytes) {
                    return Ok(downscale_cell(&img.to_rgba8()));
                }
            }
        }
    }

    // Fallback to fetch and save to cache
    for name in &filenames {
        let url = format!("{MOJI_BASE}/{name}.png");
        if let Ok(resp) = client.get(&url).send() {
            if resp.status().is_success() {
                let bytes = resp.bytes()?;

                // Write to cache
                let cache_path = cache_dir.join(format!("{}.png", name));
                let _ = fs::write(&cache_path, &bytes);

                let img = image::load_from_memory(&bytes)?.to_rgba8();
                return Ok(downscale_cell(&img));
            }
        }
    }
    Err("moji CDN miss".into())
}

fn downscale_cell(img: &RgbaImage) -> RgbaImage {
    if img.width() == CELL_PX && img.height() == CELL_PX {
        return img.clone();
    }
    image::imageops::resize(img, CELL_PX, CELL_PX, FilterType::Lanczos3)
}

fn moji_filenames(emoji: &str) -> Vec<String> {
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

#[allow(clippy::type_complexity)]
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

fn write_webp(
    atlas: &RgbaImage,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
