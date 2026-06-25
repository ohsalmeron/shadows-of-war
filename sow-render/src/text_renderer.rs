use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::context::RenderContext;
use blade_graphics as gpu;
use bytemuck::{Pod, Zeroable};

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

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, blade_macros::Vertex)]
pub struct TextInstanceGpu {
    pub screen_pos: [f32; 2],
    pub size: [f32; 2],
    pub uv_rect: [f32; 4],
    pub color: [f32; 4],
    pub outline_color: [f32; 4],
    pub outline_width_px: f32,
    pub glow_width_px: f32,
}

#[derive(blade_macros::ShaderData)]
struct TextShaderData {
    globals: TextGlobals,
    font_atlas: gpu::TextureView,
    font_sampler: gpu::Sampler,
}

pub struct FontAtlasTexture {
    pub texture: gpu::Texture,
    pub view: gpu::TextureView,
    pub buffer: gpu::Buffer,
    pub width: u32,
    pub height: u32,
}

impl FontAtlasTexture {
    pub fn new(context: &gpu::Context) -> Self {
        let png_bytes = include_bytes!("../../assets/static/fonts/msdf-atlas.png");
        let img = image::load_from_memory(png_bytes)
            .expect("Failed to load MSDF atlas PNG")
            .to_rgba8();
        let (width, height) = img.dimensions();
        let bytes_per_row = width * 4;
        let total = (bytes_per_row * height) as usize;

        let buffer = context.create_buffer(gpu::BufferDesc {
            name: "font_atlas_upload_buffer",
            size: total as u64,
            memory: gpu::Memory::Upload,
        });
        let dst = buffer.data();
        let slice = unsafe { std::slice::from_raw_parts_mut(dst, total) };
        slice.copy_from_slice(&img);
        context.sync_buffer(buffer, 0, buffer.size());

        let texture = context.create_texture(gpu::TextureDesc {
            name: "font_atlas_texture",
            format: gpu::TextureFormat::Rgba8Unorm,
            size: gpu::Extent {
                width,
                height,
                depth: 1,
            },
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
                name: "font_atlas_view",
                format: gpu::TextureFormat::Rgba8Unorm,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );

        Self {
            texture,
            view,
            buffer,
            width,
            height,
        }
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
    pipeline: gpu::RenderPipeline,
    buffer: gpu::Buffer,
    sampler: gpu::Sampler,
    pub upload_instances: Vec<TextInstanceGpu>,
}

impl TextRenderer {
    pub fn new(context: &gpu::Context, surface_format: gpu::TextureFormat) -> Self {
        let font_atlas_desc = FontAtlas::load_static();
        let font_atlas_tex = FontAtlasTexture::new(context);

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

        Self {
            font_atlas_desc,
            font_atlas_tex,
            pipeline,
            buffer,
            sampler,
            upload_instances: Vec::with_capacity(4096),
        }
    }

    pub fn upload_atlas(&self, encoder: &mut gpu::CommandEncoder, context: &gpu::Context) {
        self.font_atlas_tex.upload(encoder, context);
    }

    pub fn begin_frame(&mut self) {
        self.upload_instances.clear();
    }

    pub fn push_glyph(&mut self, inst: TextInstanceGpu) {
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
        outline_width_px: f32,
        glow_width_px: f32,
        align_x: f32,
    ) {
        if text.is_empty() {
            return;
        }

        let base_size = 48.0;
        let scale = font_size / base_size;
        let mut x_advance = 0.0f32;
        let mut glyphs_to_draw = Vec::new();
        let mut prev_char = None;

        for ch in text.chars() {
            if let Some(glyph) = self.font_atlas_desc.char_map.get(&ch).cloned() {
                let kern = prev_char
                    .and_then(|p| self.font_atlas_desc.kerning_map.get(&(p, ch)))
                    .copied()
                    .unwrap_or(0) as f32;
                let x_offset = (glyph.xoffset as f32 + kern) * scale;
                let char_x = x_advance + x_offset;
                x_advance += (glyph.xadvance as f32 + kern) * scale;
                prev_char = Some(ch);
                glyphs_to_draw.push((glyph, char_x));
            }
        }

        let align_offset = x_advance * align_x;
        let aw = self.font_atlas_tex.width as f32;
        let ah = self.font_atlas_tex.height as f32;

        for (glyph, char_x) in glyphs_to_draw {
            let gw = glyph.width as f32 * scale;
            let gh = glyph.height as f32 * scale;
            let y_off = glyph.yoffset as f32 * scale;
            let gx = pos[0] + char_x - align_offset;
            let gy = pos[1] + y_off;

            let uv_rect = [
                glyph.x as f32 / aw,
                glyph.y as f32 / ah,
                (glyph.x + glyph.width) as f32 / aw,
                (glyph.y + glyph.height) as f32 / ah,
            ];

            self.push_glyph(TextInstanceGpu {
                screen_pos: [gx, gy],
                size: [gw, gh],
                uv_rect,
                color,
                outline_color,
                outline_width_px,
                glow_width_px,
            });
        }
    }

    fn write_buffers(&self, context: &gpu::Context) {
        if !self.upload_instances.is_empty() {
            let bytes = bytemuck::cast_slice(&self.upload_instances);
            let dst = self.buffer.data();
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
            }
            context.sync_buffer(self.buffer, 0, self.buffer.size());
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
        render_ctx.context.destroy_texture_view(self.font_atlas_tex.view);
        render_ctx.context.destroy_texture(self.font_atlas_tex.texture);
        render_ctx.context.destroy_buffer(self.font_atlas_tex.buffer);
    }
}
