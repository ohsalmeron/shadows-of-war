use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::context::RenderContext;
use blade_graphics as gpu;
use bytemuck::{Pod, Zeroable};

/// Look up an emoji's UV rect in the color emoji atlas (832×768). Returns `None` if the
/// emoji isn't packed in the atlas, so callers can fall back.
pub fn emoji_uv_opt(emoji: &str) -> Option<[f32; 4]> {
    sow_data::emoji::lookup(emoji).map(|r| {
        [
            r.x as f32 / 832.0,
            r.y as f32 / 768.0,
            (r.x + r.w) as f32 / 832.0,
            (r.y + r.h) as f32 / 768.0,
        ]
    })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MsdfChar {
    pub id: u32,
    pub index: u32,
    pub char: String,
    pub width: u32,
    pub height: u32,
    pub xoffset: i32,
    pub yoffset: i32,
    pub xadvance: u32,
    pub chnl: u32,
    pub x: u32,
    pub y: u32,
    pub page: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MsdfInfo {
    pub size: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MsdfCommon {
    #[serde(rename = "lineHeight")]
    pub line_height: u32,
    pub base: u32,
    #[serde(rename = "scaleW")]
    pub scale_w: u32,
    #[serde(rename = "scaleH")]
    pub scale_h: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MsdfDistanceField {
    #[serde(rename = "fieldType")]
    pub field_type: String,
    #[serde(rename = "distanceRange")]
    pub distance_range: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MsdfKerning {
    pub first: u32,
    pub second: u32,
    pub amount: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MsdfAtlas {
    pub pages: Vec<String>,
    pub chars: Vec<MsdfChar>,
    pub info: MsdfInfo,
    pub common: MsdfCommon,
    #[serde(rename = "distanceField")]
    pub distance_field: MsdfDistanceField,
    pub kernings: Vec<MsdfKerning>,
}

pub struct FontAtlas {
    pub atlas: MsdfAtlas,
    pub char_map: HashMap<char, MsdfChar>,
    pub kerning_map: HashMap<(char, char), i32>,
}

impl FontAtlas {
    pub fn load_static() -> Self {
        let json_str = include_str!("../../assets/static/fonts/msdf-atlas.json");
        let atlas: MsdfAtlas = serde_json::from_str(json_str).expect("Failed to parse MSDF atlas JSON");
        let mut char_map = HashMap::new();
        for c in &atlas.chars {
            if let Some(first_char) = c.char.chars().next() {
                char_map.insert(first_char, c.clone());
            }
        }
        let mut kerning_map = HashMap::new();
        let char_by_id: HashMap<u32, char> = atlas.chars.iter()
            .filter_map(|c| c.char.chars().next().map(|ch| (c.id, ch)))
            .collect();
        for k in &atlas.kernings {
            if let (Some(&c1), Some(&c2)) = (char_by_id.get(&k.first), char_by_id.get(&k.second)) {
                kerning_map.insert((c1, c2), k.amount);
            }
        }
        Self {
            atlas,
            char_map,
            kerning_map,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct TextGlobals {
    pub screen_size: [f32; 2],
    pub _pad: [f32; 2],
}

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub struct TmpFontSettings {
    pub face_dilate: f32,
    pub outline_thickness: f32,
    pub underlay_offset_y: f32,
    pub underlay_softness: f32,
}

impl Default for TmpFontSettings {
    fn default() -> Self {
        Self {
            face_dilate: 0.0,
            outline_thickness: 0.8,
            underlay_offset_y: 1.5,
            underlay_softness: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, blade_macros::Vertex)]
pub struct TextInstanceGpu {
    pub screen_pos: [f32; 2],
    pub size: [f32; 2],
    pub uv_rect: [f32; 4],
    pub color: [f32; 4],
    pub outline_color: [f32; 4],
    pub face_dilate: f32,
    pub outline_thickness: f32,
    pub underlay_offset_y: f32,
    pub underlay_softness: f32,
    pub is_emoji: f32,
}

#[derive(blade_macros::ShaderData)]
struct TextShaderData {
    globals: TextGlobals,
    font_atlas: gpu::TextureView,
    font_sampler: gpu::Sampler,
    emoji_atlas: gpu::TextureView,
    emoji_sampler: gpu::Sampler,
}

pub struct FontAtlasTexture {
    pub texture: gpu::Texture,
    pub view: gpu::TextureView,
    pub buffer: gpu::Buffer,
    pub width: u32,
    pub height: u32,
}

impl FontAtlasTexture {
    pub fn from_bytes(context: &gpu::Context, png_bytes: &[u8], name: &str, format: gpu::TextureFormat) -> Self {
        let img = image::load_from_memory(png_bytes)
            .expect("Failed to load atlas PNG")
            .to_rgba8();
        let (width, height) = img.dimensions();
        let bytes_per_row = width * 4;
        let total = (bytes_per_row * height) as usize;

        let buffer = context.create_buffer(gpu::BufferDesc {
            name: &format!("{}_upload_buffer", name),
            size: total as u64,
            memory: gpu::Memory::Upload,
        });
        let dst = buffer.data();
        let slice = unsafe { std::slice::from_raw_parts_mut(dst, total) };
        slice.copy_from_slice(&img);
        context.sync_buffer(buffer, 0, buffer.size());

        let texture = context.create_texture(gpu::TextureDesc {
            name: &format!("{}_texture", name),
            format,
            size: gpu::Extent { width, height, depth: 1 },
            dimension: gpu::TextureDimension::D2,
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            usage: gpu::TextureUsage::COPY | gpu::TextureUsage::RESOURCE,
            external: None,
        });

        let view = context.create_texture_view(
            texture,
            gpu::TextureViewDesc {
                name: &format!("{}_view", name),
                format,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );

        Self { texture, view, buffer, width, height }
    }

    pub fn new(context: &gpu::Context) -> Self {
        let png_bytes = include_bytes!("../../assets/static/fonts/msdf-atlas.png");
        // ponytail: font atlas contains signed distance values (linear data), emoji atlas contains sRGB colors
        Self::from_bytes(context, png_bytes, "font_atlas", gpu::TextureFormat::Rgba8Unorm)
    }

    pub fn upload(&self, encoder: &mut gpu::CommandEncoder, context: &gpu::Context) {
        let bytes_per_row = self.width * 4;
        context.sync_buffer(self.buffer, 0, self.buffer.size());
        let src: gpu::BufferPiece = self.buffer.into();
        let dst: gpu::TexturePiece = self.texture.into();
        let mut transfer = encoder.transfer("font_atlas_upload");
        transfer.copy_buffer_to_texture(
            src,
            bytes_per_row,
            dst,
            gpu::Extent {
                width: self.width,
                height: self.height,
                depth: 1,
            },
        );
    }
}

pub const MAX_TEXT_GLYPHS: usize = 32_768;

pub struct TextRenderer {
    pub font_atlas_desc: FontAtlas,
    pub font_atlas_tex: FontAtlasTexture,
    emoji_atlas_tex: FontAtlasTexture,
    pipeline: gpu::RenderPipeline,
    buffer: gpu::Buffer,
    sampler: gpu::Sampler,
    emoji_sampler: gpu::Sampler,
    pub upload_instances: Vec<TextInstanceGpu>,
}

impl TextRenderer {
    pub fn new(context: &gpu::Context, surface_format: gpu::TextureFormat) -> Self {
        let font_atlas_desc = FontAtlas::load_static();
        let font_atlas_tex = FontAtlasTexture::new(context);
        let emoji_atlas_tex = FontAtlasTexture::from_bytes(
            context,
            crate::EMOJI_ATLAS_BYTES,
            "emoji_atlas",
            gpu::TextureFormat::Rgba8UnormSrgb, // ponytail: hardware sRGB decode
        );

        let shader_source = include_str!("shaders/text_glow.wgsl");
        let shader = context.create_shader(gpu::ShaderDesc {
            source: shader_source,
            naga_module: None,
        });
        assert_eq!(
            std::mem::size_of::<TextGlobals>(),
            shader.get_struct_size("TextGlobals") as usize,
        );

        let text_layout = <TextShaderData as gpu::ShaderData>::layout();
        let text_vertex_layout = <TextInstanceGpu as gpu::Vertex>::layout();
        let blend = gpu::BlendState {
            color: gpu::BlendComponent {
                src_factor: gpu::BlendFactor::SrcAlpha,
                dst_factor: gpu::BlendFactor::OneMinusSrcAlpha,
                operation: gpu::BlendOperation::Add,
            },
            alpha: gpu::BlendComponent::OVER,
        };

        let pipeline = context.create_render_pipeline(gpu::RenderPipelineDesc {
            name: "text_glow_pipeline",
            data_layouts: &[&text_layout],
            vertex: shader.at("vs_main"),
            vertex_fetches: &[gpu::VertexFetchState {
                layout: &text_vertex_layout,
                instanced: true,
            }],
            primitive: gpu::PrimitiveState::default(),
            depth_stencil: None,
            fragment: Some(shader.at("fs_main")),
            color_targets: &[gpu::ColorTargetState {
                format: surface_format,
                blend: Some(blend),
                write_mask: gpu::ColorWrites::default(),
            }],
            multisample_state: gpu::MultisampleState::default(),
        });

        let buffer = context.create_buffer(gpu::BufferDesc {
            name: "text_glow_instances",
            size: (MAX_TEXT_GLYPHS * std::mem::size_of::<TextInstanceGpu>()) as u64,
            memory: gpu::Memory::Upload,
        });

        let sampler = context.create_sampler(gpu::SamplerDesc {
            name: "font_atlas_sampler",
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });

        let emoji_sampler = context.create_sampler(gpu::SamplerDesc {
            name: "emoji_atlas_sampler",
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            font_atlas_desc,
            font_atlas_tex,
            emoji_atlas_tex,
            pipeline,
            buffer,
            sampler,
            emoji_sampler,
            upload_instances: Vec::with_capacity(4096),
        }
    }

    pub fn upload_atlas(&self, encoder: &mut gpu::CommandEncoder, context: &gpu::Context) {
        self.font_atlas_tex.upload(encoder, context);
        self.emoji_atlas_tex.upload(encoder, context);
    }

    pub fn begin_frame(&mut self) {
        self.upload_instances.clear();
    }

    pub fn push_glyph(&mut self, inst: TextInstanceGpu) {
        if self.upload_instances.len() < MAX_TEXT_GLYPHS {
            self.upload_instances.push(inst);
        }
    }

    /// Internal: push a glyph or emoji instance, respecting buffer limits.
    fn push_inst(&mut self, inst: TextInstanceGpu) {
        if self.upload_instances.len() < MAX_TEXT_GLYPHS {
            self.upload_instances.push(inst);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn push_string(
        &mut self,
        text: &str,
        pos: [f32; 2],
        font_size: f32,
        color: [f32; 4],
        outline_color: [f32; 4],
        settings: TmpFontSettings,
        align_x: f32,
        char_spacing: f32,
        emoji_scale: f32,
    ) {
        if text.is_empty() {
            return;
        }

        let scale = font_size / 48.0;
        let aw = self.font_atlas_tex.width as f32;
        let ah = self.font_atlas_tex.height as f32;
        let base = self.font_atlas_desc.atlas.common.base as f32;

        // Real zero-allocation layout: emit instances straight into the persistent
        // `upload_instances` buffer (already capacity-reserved + cleared per frame),
        // remember where this string started, then apply horizontal alignment as a
        // single in-place shift. No per-call scratch Vec — see README "Zero-Allocation
        // Hot Path". Replaces the mislabeled with_capacity(32)/(8) per-call heap allocs.
        let start = self.upload_instances.len();
        let mut x_advance = 0.0f32;
        let mut prev_char = Option::<char>::None;
        let mut chars = text.char_indices().peekable();

        while let Some((byte_idx, ch)) = chars.next() {
            if let Some(glyph) = self.font_atlas_desc.char_map.get(&ch) {
                let kern = prev_char
                    .and_then(|p| self.font_atlas_desc.kerning_map.get(&(p, ch)))
                    .copied()
                    .unwrap_or(0) as f32;
                let char_x = x_advance + (glyph.xoffset as f32 + kern) * scale;
                x_advance += (glyph.xadvance as f32 + kern) * scale * char_spacing;
                prev_char = Some(ch);
                // Disjoint field borrow: `glyph` borrows `font_atlas_desc` while we
                // push to `upload_instances` (a different field), so no intermediate
                // buffer is needed to satisfy the borrow checker.
                if self.upload_instances.len() < MAX_TEXT_GLYPHS {
                    let gw = glyph.width as f32 * scale;
                    let gh = glyph.height as f32 * scale;
                    let y_off = glyph.yoffset as f32 * scale;
                    self.upload_instances.push(TextInstanceGpu {
                        screen_pos: [pos[0] + char_x, pos[1] - base * scale + y_off],
                        size: [gw, gh],
                        uv_rect: [
                            glyph.x as f32 / aw,
                            glyph.y as f32 / ah,
                            (glyph.x + glyph.width) as f32 / aw,
                            (glyph.y + glyph.height) as f32 / ah,
                        ],
                        color,
                        outline_color,
                        face_dilate: settings.face_dilate,
                        outline_thickness: settings.outline_thickness,
                        underlay_offset_y: settings.underlay_offset_y,
                        underlay_softness: settings.underlay_softness,
                        is_emoji: 0.0,
                    });
                }
                continue;
            }
            let has_selector = chars.peek().map_or(false, |&(_, next_ch)| next_ch == '\u{fe0f}');
            let char_len = ch.len_utf8();
            let total_len = if has_selector {
                char_len + '\u{fe0f}'.len_utf8()
            } else {
                char_len
            };
            let candidate = &text[byte_idx..byte_idx + total_len];
            let stripped = if has_selector {
                &text[byte_idx..byte_idx + char_len]
            } else {
                candidate
            };
            if let Some(uv) = emoji_uv_opt(stripped) {
                let emoji_size = font_size * emoji_scale;
                let advance = x_advance;
                x_advance += emoji_size * char_spacing;
                prev_char = None;
                if self.upload_instances.len() < MAX_TEXT_GLYPHS {
                    self.upload_instances.push(TextInstanceGpu {
                        screen_pos: [pos[0] + advance, pos[1] - emoji_size],
                        size: [emoji_size, emoji_size],
                        uv_rect: uv,
                        color,
                        outline_color,
                        face_dilate: 0.0,
                        outline_thickness: settings.outline_thickness,
                        underlay_offset_y: settings.underlay_offset_y,
                        underlay_softness: 0.0,
                        is_emoji: 1.0,
                    });
                }
                if has_selector {
                    chars.next();
                }
                continue;
            }
            prev_char = None;
        }

        // Alignment is one cheap in-place pass over the instances we just emitted,
        // replacing the old second buffer-building loop.
        let align_offset = x_advance * align_x;
        if align_offset != 0.0 {
            for inst in &mut self.upload_instances[start..] {
                inst.screen_pos[0] -= align_offset;
            }
        }
    }

    /// Push a screen-space emoji with alpha-dilated outline + drop shadow.
    /// `screen_pos` is the center in physical pixels, `half_size` the half-extent.
    /// Returns `false` if the emoji isn't in the atlas.
    pub fn push_emoji(
        &mut self,
        emoji: &str,
        screen_pos: [f32; 2],
        half_size: f32,
        tint: [f32; 4],
        outline_color: [f32; 4],
        outline_thickness: f32,
        shadow_offset_y: f32,
    ) -> bool {
        let Some(emoji_uv) = emoji_uv_opt(emoji) else {
            return false;
        };
        // Expand quad 25% so outline taps have room outside the sprite.
        let expand = 1.25;
        let expanded = half_size * expand;
        let top_left = [screen_pos[0] - expanded, screen_pos[1] - expanded];
        self.push_inst(TextInstanceGpu {
            screen_pos: top_left,
            size: [expanded * 2.0; 2],
            uv_rect: emoji_uv,
            color: tint,
            outline_color,
            face_dilate: 0.0,
            outline_thickness,
            underlay_offset_y: shadow_offset_y,
            underlay_softness: 0.0,
            is_emoji: 1.0,
        });
        true
    }

    fn write_buffers(&self, context: &gpu::Context) {
        if !self.upload_instances.is_empty() {
            let bytes = bytemuck::cast_slice(&self.upload_instances);
            let dst = self.buffer.data();
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
            }
            // ponytail: only sync active slice to avoid massive WASM/WebGL overhead
            context.sync_buffer(self.buffer, 0, bytes.len() as u64);
        }
    }

    pub fn draw(
        &mut self,
        encoder: &mut gpu::CommandEncoder,
        target_view: gpu::TextureView,
        screen_size: [f32; 2],
        context: &gpu::Context,
    ) {
        let glyph_count = self.upload_instances.len() as u32;
        if glyph_count == 0 {
            return;
        }

        self.write_buffers(context);

        let mut pass = encoder.render(
            "text_pass",
            gpu::RenderTargetSet {
                colors: &[gpu::RenderTarget {
                    view: target_view,
                    init_op: gpu::InitOp::Load,
                    finish_op: gpu::FinishOp::Store,
                }],
                depth_stencil: None,
            },
        );

        let globals = TextGlobals {
            screen_size,
            _pad: [0.0; 2],
        };

        let shader_data = TextShaderData {
            globals,
            font_atlas: self.font_atlas_tex.view,
            font_sampler: self.sampler,
            emoji_atlas: self.emoji_atlas_tex.view,
            emoji_sampler: self.emoji_sampler,
        };

        let mut rc = pass.with(&self.pipeline);
        rc.bind(0, &shader_data);
        rc.bind_vertex(0, self.buffer.at(0));
        rc.draw(0, 6, 0, glyph_count);
    }

    pub fn destroy(&mut self, render_ctx: &RenderContext) {
        render_ctx.context.destroy_render_pipeline(&mut self.pipeline);
        render_ctx.context.destroy_buffer(self.buffer);
        render_ctx.context.destroy_sampler(self.sampler);
        render_ctx.context.destroy_sampler(self.emoji_sampler);
        render_ctx.context.destroy_texture_view(self.font_atlas_tex.view);
        render_ctx.context.destroy_texture(self.font_atlas_tex.texture);
        render_ctx.context.destroy_buffer(self.font_atlas_tex.buffer);
        render_ctx.context.destroy_texture_view(self.emoji_atlas_tex.view);
        render_ctx.context.destroy_texture(self.emoji_atlas_tex.texture);
        render_ctx.context.destroy_buffer(self.emoji_atlas_tex.buffer);
    }
}
