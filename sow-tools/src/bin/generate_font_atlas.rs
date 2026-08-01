//! Generate a single-channel SDF font atlas from a .ttf file.
//!
//! Usage: cargo run --bin generate-font-atlas -- <ttf_path> <out_png> <out_json>
//! Example: cargo run --bin generate-font-atlas -- assets/static/fonts/WorkSans-Black.ttf assets/static/fonts/msdf-atlas.png assets/static/fonts/msdf-atlas.json

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};

const FONT_SIZE: f32 = 48.0;
const DISTANCE_RANGE: f32 = 16.0;
const PADDING: u32 = 8;
const ATLAS_SIZE: u32 = 1024;
/// Supersampling factor for rasterization before the SDF is downsampled.
const UPSAMPLE: u32 = 4;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: {} <ttf_path> <out_png> <out_json>", args[0]);
        std::process::exit(1);
    }
    let ttf_path = &args[1];
    let out_png = &args[2];
    let out_json = &args[3];

    let ttf_data = std::fs::read(ttf_path).expect("Failed to read TTF");
    let font = FontRef::try_from_slice(&ttf_data).expect("Failed to parse TTF");

    let scale = PxScale::from(FONT_SIZE);
    let scaled = font.as_scaled(scale);
    // Higher-resolution scale used only for rasterizing the coverage mask.
    let hi_scale = PxScale::from(FONT_SIZE * UPSAMPLE as f32);

    let ascent = scaled.ascent();
    let descent = scaled.descent();
    let line_gap = scaled.line_gap();
    let line_height = (ascent - descent + line_gap).ceil() as u32;
    let base = (ascent).ceil() as u32;

    let face_name = std::path::Path::new(ttf_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // Collect all printable ASCII glyphs
    let mut glyphs: Vec<(char, u32)> = Vec::new();
    for codepoint in 32u32..=126 {
        let ch = char::from_u32(codepoint).unwrap();
        let glyph_id = font.glyph_id(ch);
        glyphs.push((ch, glyph_id.0 as u32));
    }

    eprintln!(
        "Rasterizing {} glyphs at font size {}...",
        glyphs.len(),
        FONT_SIZE
    );

    struct GlyphSheet {
        codepoint: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        xoffset: i32,
        yoffset: i32,
        xadvance: u32,
        pixels: Vec<u8>, // RGB triplets
    }

    let mut sheets: Vec<GlyphSheet> = Vec::new();

    for (ch, _glyph_id) in &glyphs {
        let gid = font.glyph_id(*ch);
        let xadvance = scaled.h_advance(gid).round() as u32;

        // Tight ink bounds at the base size define the cell and the BMFont offsets.
        // (Ink-less glyphs such as space have no outline.)
        let bounds = font
            .outline_glyph(gid.with_scale(scale))
            .map(|o| o.px_bounds());

        let (cell_w, cell_h, xoffset, yoffset) = match bounds {
            Some(b) => {
                let cw = (b.width().ceil() as u32).max(1) + PADDING * 2;
                let ch_ = (b.height().ceil() as u32).max(1) + PADDING * 2;
                // xoffset: pen position -> left edge of the padded cell.
                let xo = b.min.x.round() as i32 - PADDING as i32;
                // yoffset: line top -> top of the padded cell. The renderer draws at
                // `baseline - base + yoffset`, and `b.min.y` is the ink top relative to
                // the baseline (negative above it), so yoffset = base + ink_top - PADDING.
                let yo = base as i32 + b.min.y.round() as i32 - PADDING as i32;
                (cw, ch_, xo, yo)
            }
            None => (
                PADDING * 2 + 1,
                PADDING * 2 + 1,
                -(PADDING as i32),
                base as i32 - PADDING as i32,
            ),
        };

        let raster_w = cell_w as usize * UPSAMPLE as usize;
        let raster_h = cell_h as usize * UPSAMPLE as usize;

        // Rasterize the glyph at UPSAMPLE resolution so the distance field captures
        // full stroke widths, then downsample. Ink is aligned PADDING texels in.
        let mut mask = vec![0u8; raster_w * raster_h];
        if let Some(outlined) = font.outline_glyph(gid.with_scale(hi_scale)) {
            let off = (PADDING * UPSAMPLE) as i32;
            outlined.draw(|px, py, coverage| {
                let x = px as i32 + off;
                let y = py as i32 + off;
                if x >= 0 && y >= 0 && (x as usize) < raster_w && (y as usize) < raster_h {
                    let idx = y as usize * raster_w + x as usize;
                    mask[idx] = mask[idx].max((coverage * 255.0) as u8);
                }
            });
        }

        // Compute SDF: 2-pass chamfer distance transform at the upsampled resolution.
        let sdf = compute_sdf(&mask, raster_w, raster_h, DISTANCE_RANGE * UPSAMPLE as f32);

        // Downsample to cell size (exactly UPSAMPLE:1 in each axis).
        let mut pixels = Vec::with_capacity((cell_w * cell_h * 3) as usize);
        for cy in 0..cell_h as usize {
            for cx in 0..cell_w as usize {
                let mut sum = 0.0f32;
                let mut count = 0u32;
                for dy in 0..UPSAMPLE as usize {
                    for dx in 0..UPSAMPLE as usize {
                        let sx = cx * UPSAMPLE as usize + dx;
                        let sy = cy * UPSAMPLE as usize + dy;
                        if sx < raster_w && sy < raster_h {
                            sum += sdf[sy * raster_w + sx];
                            count += 1;
                        }
                    }
                }
                let avg = if count > 0 { sum / count as f32 } else { 0.0 };
                // [0,1] range where 0.5 = glyph edge. msdfgen convention: inside > 0.5
                // (bright). `avg` is a signed distance in upsampled raster pixels, so
                // divide by DISTANCE_RANGE * UPSAMPLE to express it in atlas texels,
                // matching the `distanceRange` in the JSON and the shader.
                let val = (avg / (DISTANCE_RANGE * UPSAMPLE as f32) + 0.5).clamp(0.0, 1.0);
                let byte = (val * 255.0) as u8;
                // Duplicate across RGB (single-channel SDF).
                pixels.push(byte);
                pixels.push(byte);
                pixels.push(byte);
            }
        }

        sheets.push(GlyphSheet {
            codepoint: *ch as u32,
            x: 0,
            y: 0,
            width: cell_w,
            height: cell_h,
            xoffset,
            yoffset,
            xadvance,
            pixels,
        });
    }

    // Pack glyphs into atlas using shelf-based bin packing
    eprintln!("Packing glyphs into {}x{} atlas...", ATLAS_SIZE, ATLAS_SIZE);
    let mut atlas_pixels = vec![0u8; ATLAS_SIZE as usize * ATLAS_SIZE as usize * 3];

    let mut cursor_x = 0u32;
    let mut cursor_y = 0u32;
    let mut row_height = 0u32;

    for sheet in &mut sheets {
        if cursor_x + sheet.width > ATLAS_SIZE {
            cursor_x = 0;
            cursor_y += row_height;
            row_height = 0;
        }
        if cursor_y + sheet.height > ATLAS_SIZE {
            eprintln!("ERROR: Atlas overflow! Increase ATLAS_SIZE.");
            std::process::exit(1);
        }
        sheet.x = cursor_x;
        sheet.y = cursor_y;

        // Copy glyph pixels to atlas
        for gy in 0..sheet.height {
            for gx in 0..sheet.width {
                let src_idx = ((gy * sheet.width + gx) * 3) as usize;
                let dst_x = cursor_x + gx;
                let dst_y = cursor_y + gy;
                let dst_idx = ((dst_y * ATLAS_SIZE + dst_x) * 3) as usize;
                atlas_pixels[dst_idx..dst_idx + 3]
                    .copy_from_slice(&sheet.pixels[src_idx..src_idx + 3]);
            }
        }

        cursor_x += sheet.width;
        row_height = row_height.max(sheet.height);
    }

    // Write PNG
    eprintln!("Writing atlas PNG: {}", out_png);
    let img = image::RgbImage::from_raw(ATLAS_SIZE, ATLAS_SIZE, atlas_pixels)
        .expect("Failed to create atlas image");
    img.save(out_png).expect("Failed to save atlas PNG");

    // Build JSON
    eprintln!("Writing atlas JSON: {}", out_json);
    let mut chars_json: Vec<serde_json::Value> = Vec::new();
    for (idx, sheet) in sheets.iter().enumerate() {
        chars_json.push(serde_json::json!({
            "id": sheet.codepoint,
            "index": idx,
            "char": char::from_u32(sheet.codepoint).unwrap().to_string(),
            "width": sheet.width,
            "height": sheet.height,
            "xoffset": sheet.xoffset,
            "yoffset": sheet.yoffset,
            "xadvance": sheet.xadvance,
            "chnl": 15, // all 4 channels (R|G|B|A bitfield)
            "x": sheet.x,
            "y": sheet.y,
            "page": 0,
        }));
    }

    let charset: Vec<String> = glyphs.iter().map(|(ch, _)| ch.to_string()).collect();

    let atlas_json = serde_json::json!({
        "pages": [format!("{}.png", face_name)],
        "chars": chars_json,
        "info": {
            "face": face_name,
            "size": FONT_SIZE as u32,
            "bold": 0,
            "italic": 0,
            "charset": charset,
            "unicode": 1,
            "stretchH": 100,
            "smooth": 1,
            "aa": 1,
            "padding": [PADDING, PADDING, PADDING, PADDING],
            "spacing": [0, 0],
            "outline": 0,
        },
        "common": {
            "lineHeight": line_height,
            "base": base,
            "scaleW": ATLAS_SIZE,
            "scaleH": ATLAS_SIZE,
            "pages": 1,
            "packed": 0,
            "alphaChnl": 0,
            "redChnl": 0,
            "greenChnl": 0,
            "blueChnl": 0,
        },
        "distanceField": {
            "fieldType": "msdf",
            "distanceRange": DISTANCE_RANGE,
        },
        "kernings": [],
    });

    let json_str = serde_json::to_string_pretty(&atlas_json).expect("Failed to serialize JSON");
    std::fs::write(out_json, json_str).expect("Failed to write JSON");

    eprintln!("Done! {} glyphs packed.", sheets.len());
}

fn compute_sdf(mask: &[u8], w: usize, h: usize, spread: f32) -> Vec<f32> {
    let total = w * h;
    let mut outside = vec![f32::INFINITY; total];
    let mut inside = vec![f32::INFINITY; total];

    // Initialize: edge pixels get distance 0
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let idx = y * w + x;
            let is_filled = mask[idx] > 127;
            let is_edge = is_filled
                && (mask[idx - 1] <= 127
                    || mask[idx + 1] <= 127
                    || mask[idx - w] <= 127
                    || mask[idx + w] <= 127);
            if is_edge
                || (!is_filled
                    && (mask[idx - 1] > 127
                        || mask[idx + 1] > 127
                        || mask[idx - w] > 127
                        || mask[idx + w] > 127))
            {
                outside[idx] = 0.0;
                inside[idx] = 0.0;
            }
        }
    }

    // 2-pass dead reckoning for squared Euclidean distance
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let idx = y * w + x;
            if mask[idx] > 127 {
                // Inside glyph
                let d = inside[idx];
                let d1 = inside[idx - 1] + 1.0;
                let d2 = inside[idx - w] + 1.0;
                let d3 = inside[idx - w - 1] + 1.414;
                let d4 = inside[idx - w + 1] + 1.414;
                inside[idx] = d.min(d1).min(d2).min(d3).min(d4);
            } else {
                // Outside glyph
                let d = outside[idx];
                let d1 = outside[idx - 1] + 1.0;
                let d2 = outside[idx - w] + 1.0;
                let d3 = outside[idx - w - 1] + 1.414;
                let d4 = outside[idx - w + 1] + 1.414;
                outside[idx] = d.min(d1).min(d2).min(d3).min(d4);
            }
        }
    }

    for y in (1..h - 1).rev() {
        for x in (1..w - 1).rev() {
            let idx = y * w + x;
            if mask[idx] > 127 {
                let d = inside[idx];
                let d1 = inside[idx + 1] + 1.0;
                let d2 = inside[idx + w] + 1.0;
                let d3 = inside[idx + w - 1] + 1.414;
                let d4 = inside[idx + w + 1] + 1.414;
                inside[idx] = d.min(d1).min(d2).min(d3).min(d4);
            } else {
                let d = outside[idx];
                let d1 = outside[idx + 1] + 1.0;
                let d2 = outside[idx + w] + 1.0;
                let d3 = outside[idx + w - 1] + 1.414;
                let d4 = outside[idx + w + 1] + 1.414;
                outside[idx] = d.min(d1).min(d2).min(d3).min(d4);
            }
        }
    }

    // Combine into signed distance field
    let mut sdf = vec![0.0f32; total];
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let dist_out = outside[idx];
            let dist_in = inside[idx];
            // Standard SDF convention: positive inside, negative outside.
            // This makes the glyph interior bright (> 0.5) and the background dark,
            // which is what the MSDF/SDF shader (median + screenPxRange) expects.
            if mask[idx] > 127 {
                sdf[idx] = dist_in.min(spread);
            } else {
                sdf[idx] = -dist_out.min(spread);
            }
        }
    }

    sdf
}
