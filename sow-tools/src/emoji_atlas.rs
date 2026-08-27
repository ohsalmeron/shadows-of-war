use image::imageops::FilterType;
use image::{ImageBuffer, ImageEncoder, Rgba, RgbaImage};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const CELL_PX: u32 = 64;
const GUTTER_PX: u32 = 2;
const TILE_PX: u32 = CELL_PX + GUTTER_PX * 2; // 68px per tile (2px transparent border)
const MOJI_BASE: &str = "https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/72x72";

/// Canonical list of all gameplay, HUD, leader, tribe, and reaction emojis.
pub const CANONICAL_GAMEPLAY_EMOJIS: &[&str] = &[
    // Core Buildings & Infrastructure
    "🏛", "🏭", "⚓", "🛡",
    // Units, Movers, Combat & Weapons
    "🚢", "⛵", "⚔", "💣", "🚀", "☢", "💥", "🪖", "🏹", "🪓",
    // Resources & Economy
    "🪙", "🌾", "⚙", "🌽", "🍞", "🧂",
    // Diplomacy, Rankings & Match Status
    "👑", "⭐", "🤝", "🕊", "💔", "🏳", "🔌", "🔒", "📨", "📩", "📋", "✅", "❌", "ℹ", "⚠️", "🎖", "🏆", "🏕", "🏠",
    // Map Editor & Tools
    "⛰", "❄", "🏝", "🖌", "🗑", "📁", "🌍", "⚡", "🛠", "🔧", "📦", "🔄",
    // Tribe Animals (sow-data)
    "🐯", "🐆", "🦊", "🦝", "🐻", "🐨", "🐼", "🐗", "🦄", "🦅", "🦉", "🐊", "🦖", "🐉",
    "🦈", "🦂", "🐃", "🐏", "🐘", "🦏", "🦍", "🐎", "🦌", "🦇", "🦢", "🦩", "🐍", "🐢",
    "🐙", "🐬", "🐝", "🦋", "🕷", "🦦", "🦫", "🐫", "🦘", "🦡", "🦁",
    // In-game Reactions & Chat
    "😀", "😏", "😂", "🤣", "😋", "😉", "😜", "😍", "🥰", "🥳", "🥺", "😇", "🤩", "👍",
    "❤️", "🤔", "🧐", "🙄", "🤯", "🤡", "💩", "🤫", "😠", "🤬", "😤", "🥵", "🥶", "🤢",
    "🤮", "💀", "💪", "👀",
];

pub struct PackEmojiAtlasArgs {
    pub repo_root: PathBuf,
    pub out_atlas: PathBuf,
    pub out_manifest: PathBuf,
}

pub fn pack(args: PackEmojiAtlasArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut unique_glyphs: BTreeSet<String> = BTreeSet::new();

    // 1. Seed with all canonical gameplay emojis (normalized)
    for &e in CANONICAL_GAMEPLAY_EMOJIS {
        let norm = normalize_emoji(e);
        if !norm.is_empty() {
            unique_glyphs.insert(norm);
        }
    }

    // 2. Scan workspace sources for any additional emojis used in code/strings
    let scanned = scan_source_for_emojis(&args.repo_root)?;
    for e in scanned {
        let norm = normalize_emoji(&e);
        if !norm.is_empty() {
            unique_glyphs.insert(norm);
        }
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new());

    let mut entries: Vec<(String, RgbaImage)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    for emoji in &unique_glyphs {
        if std::env::var("VERBOSE").is_ok() {
            println!("Fetching glyph: {emoji}");
        }
        match fetch_moji(&client, emoji) {
            Ok(img) => entries.push((emoji.clone(), img)),
            Err(e) => {
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
    let packed = pack_grid(&entries, CELL_PX, GUTTER_PX)?;
    let atlas = &packed.atlas;
    write_webp(atlas, &args.out_atlas)?;
    write_manifest_rs(&packed.rects, atlas.width(), atlas.height(), &args.out_manifest)?;
    println!(
        "✅ Packed {} unique glyphs with {}px gutter → {} ({}x{})",
        entries.len(),
        GUTTER_PX,
        args.out_atlas.display(),
        atlas.width(),
        atlas.height()
    );
    Ok(())
}

fn normalize_emoji(s: &str) -> String {
    s.chars().filter(|&c| c != '\u{fe0f}').collect::<String>().trim().to_string()
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
                    || name == "cache"
                {
                    continue;
                }
                if path.parent() == Some(repo_root) && !name.starts_with("sow-") {
                    continue;
                }
            }
            for entry in fs::read_dir(path)? {
                stack.push(entry?.path());
            }
        } else {
            let ext = path.extension().and_then(|s| s.to_str());
            if ext == Some("rs") || ext == Some("toml") {
                if path.to_string_lossy().contains("manifest.rs") {
                    continue;
                }
                if let Ok(content) = fs::read_to_string(&path) {
                    let mut current_emoji = String::new();
                    for c in content.chars() {
                        let cp = c as u32;
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
                            if !trimmed.is_empty()
                                && !trimmed.chars().all(|ch| ch == '─' || ch == '═' || ch == '━')
                            {
                                out.push(current_emoji.clone());
                            }
                            current_emoji.clear();
                        }
                    }
                    if !current_emoji.is_empty() {
                        let trimmed = current_emoji.trim();
                        if !trimmed.is_empty()
                            && !trimmed.chars().all(|ch| ch == '─' || ch == '═' || ch == '━')
                        {
                            out.push(current_emoji);
                        }
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
    let cache_dir = Path::new("assets/gameplay/emoji/cache");
    fs::create_dir_all(cache_dir)?;

    let filenames = moji_filenames(emoji);
    if filenames.is_empty() {
        return Err("No filenames generated for emoji".into());
    }

    // 1. Try finding in local disk cache first
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

    // Also check legacy cache if present
    let legacy_cache = Path::new("assets/emoji/cache");
    if legacy_cache.is_dir() {
        for name in &filenames {
            let cache_path = legacy_cache.join(format!("{}.png", name));
            if cache_path.exists() {
                if let Ok(bytes) = fs::read(&cache_path) {
                    if let Ok(img) = image::load_from_memory(&bytes) {
                        return Ok(downscale_cell(&img.to_rgba8()));
                    }
                }
            }
        }
    }

    // 2. Fetch from CDN and persist to disk cache
    for name in &filenames {
        let url = format!("{MOJI_BASE}/{name}.png");
        if let Ok(resp) = client.get(&url).send() {
            if resp.status().is_success() {
                let bytes = resp.bytes()?;
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

struct PackedGlyph {
    emoji: String,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

struct PackAtlasResult {
    atlas: RgbaImage,
    rects: Vec<PackedGlyph>,
}

fn pack_grid(
    entries: &[(String, RgbaImage)],
    cell: u32,
    gutter: u32,
) -> Result<PackAtlasResult, Box<dyn std::error::Error + Send + Sync>> {
    let n = entries.len() as u32;
    let cols = (n as f32).sqrt().ceil() as u32;
    let rows = n.div_ceil(cols);
    let tile = cell + gutter * 2;
    let w = cols * tile;
    let h = rows * tile;
    let mut atlas: RgbaImage = ImageBuffer::from_pixel(w, h, Rgba([0, 0, 0, 0]));
    let mut rects = Vec::new();
    for (i, (emoji, img)) in entries.iter().enumerate() {
        let col = (i as u32) % cols;
        let row = (i as u32) / cols;
        let x = col * tile + gutter;
        let y = row * tile + gutter;
        image::imageops::overlay(&mut atlas, img, x.into(), y.into());
        rects.push(PackedGlyph {
            emoji: emoji.clone(),
            x,
            y,
            w: cell,
            h: cell,
        });
    }
    Ok(PackAtlasResult { atlas, rects })
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
    rects: &[PackedGlyph],
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
    out.push_str("pub fn lookup(emoji: &str) -> Option<AtlasRect> {\n");
    out.push_str("    if emoji.contains('\\u{fe0f}') {\n");
    out.push_str("        let clean: String = emoji.chars().filter(|&c| c != '\\u{fe0f}').collect();\n");
    out.push_str("        return match_canonical(&clean);\n");
    out.push_str("    }\n");
    out.push_str("    match_canonical(emoji)\n");
    out.push_str("}\n\n");
    out.push_str("fn match_canonical(emoji: &str) -> Option<AtlasRect> {\n");
    out.push_str("    match emoji {\n");
    for rect in rects {
        let escaped = rect.emoji.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!(
            "        \"{escaped}\" => Some(AtlasRect {{ x: {}, y: {}, w: {}, h: {} }}),\n",
            rect.x, rect.y, rect.w, rect.h
        ));
    }
    out.push_str("        _ => None,\n    }\n}\n");
    fs::write(path, out)?;
    Ok(())
}
