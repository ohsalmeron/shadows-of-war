use crate::context::RenderContext;
use blade_graphics as gpu;
use bytemuck::{Pod, Zeroable};

pub const MAX_STRUCTURE_INSTANCES: usize = 16_384;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct StructureGlobals {
    pub camera_pos: [f32; 2],
    pub zoom: f32,
    pub _pad1: f32,
    pub screen_size: [f32; 2],
    /// Bare-emoji outline thickness in physical pixels (shares the text's calibrated
    /// `dev_font_outline_thickness` value).
    pub outline_px: f32,
    /// Bare-emoji drop-shadow offset in physical pixels (shares `dev_font_shadow_y`).
    pub shadow_px: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, blade_macros::Vertex)]
pub struct StructureInstanceGpu {
    pub world_pos: [f32; 2],
    pub size: f32,
    pub shape_type: f32, // 0=circle, 1=octagon, 2=square, 3=pentagon
    pub color: [f32; 4],
    pub outline_color: [f32; 4],
    pub icon_uv: [f32; 4],
    pub opacity: f32,
}

#[derive(blade_macros::ShaderData)]
struct StructureShaderData {
    globals: StructureGlobals,
    icon_atlas: gpu::TextureView,
    icon_sampler: gpu::Sampler,
}

pub struct StructureRenderer {
    pub texture: gpu::Texture,
    pub view: gpu::TextureView,
    pub upload_buffer: gpu::Buffer,
    pub width: u32,
    pub height: u32,
    pipeline: gpu::RenderPipeline,
    buffer: gpu::Buffer,
    sampler: gpu::Sampler,
    pub upload_instances: Vec<StructureInstanceGpu>,
}

impl StructureRenderer {
    pub fn new(context: &gpu::Context, surface_format: gpu::TextureFormat) -> Self {
        // Load the full twemoji atlas natively
        let atlas_bytes = sow_assets::EMOJI_ATLAS_BYTES;
        let img = image::load_from_memory(atlas_bytes)
            .expect("Failed to load emoji atlas")
            .to_rgba8();
        let (width, height) = img.dimensions(); // 832 x 768
        let bytes_per_row = width * 4;
        let total = (bytes_per_row * height) as usize;

        let upload_buffer = context.create_buffer(gpu::BufferDesc {
            name: "struct_atlas_upload_buffer",
            size: total as u64,
            memory: gpu::Memory::Upload,
        });
        let dst = upload_buffer.data();
        let slice = unsafe { std::slice::from_raw_parts_mut(dst, total) };
        slice.copy_from_slice(&img);
        context.sync_buffer(upload_buffer, 0, upload_buffer.size());

        let texture = context.create_texture(gpu::TextureDesc {
            name: "struct_atlas_texture",
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
                name: "struct_atlas_view",
                format: gpu::TextureFormat::Rgba8Unorm,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );

        let shader_source = include_str!("shaders/structure.wgsl");
        let shader = context.create_shader(gpu::ShaderDesc {
            source: shader_source,
            naga_module: None,
        });
        assert_eq!(
            std::mem::size_of::<StructureGlobals>(),
            shader.get_struct_size("StructureGlobals") as usize,
        );

        let struct_layout = <StructureShaderData as gpu::ShaderData>::layout();
        let struct_vertex_layout = <StructureInstanceGpu as gpu::Vertex>::layout();
        let blend = gpu::BlendState {
            color: gpu::BlendComponent {
                src_factor: gpu::BlendFactor::SrcAlpha,
                dst_factor: gpu::BlendFactor::OneMinusSrcAlpha,
                operation: gpu::BlendOperation::Add,
            },
            alpha: gpu::BlendComponent::OVER,
        };

        let pipeline = context.create_render_pipeline(gpu::RenderPipelineDesc {
            name: "structure_pipeline",
            data_layouts: &[&struct_layout],
            vertex: shader.at("vs_main"),
            vertex_fetches: &[gpu::VertexFetchState {
                layout: &struct_vertex_layout,
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
            name: "structure_instances",
            size: (MAX_STRUCTURE_INSTANCES * std::mem::size_of::<StructureInstanceGpu>()) as u64,
            memory: gpu::Memory::Upload,
        });

        let sampler = context.create_sampler(gpu::SamplerDesc {
            name: "structure_atlas_sampler",
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            texture,
            view,
            upload_buffer,
            width,
            height,
            pipeline,
            buffer,
            sampler,
            upload_instances: Vec::with_capacity(4096),
        }
    }

    pub fn upload_atlas(&self, encoder: &mut gpu::CommandEncoder, context: &gpu::Context) {
        let bytes_per_row = self.width * 4;
        context.sync_buffer(self.upload_buffer, 0, self.upload_buffer.size());
        let src: gpu::BufferPiece = self.upload_buffer.into();
        let dst: gpu::TexturePiece = self.texture.into();
        let mut transfer = encoder.transfer("struct_atlas_upload");
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

    pub fn begin_frame(&mut self) {
        self.upload_instances.clear();
    }

    pub fn push_structure(&mut self, inst: StructureInstanceGpu) {
        if self.upload_instances.len() < MAX_STRUCTURE_INSTANCES {
            self.upload_instances.push(inst);
        }
    }

    /// Push a **screen-space** bare emoji (no shape) with an alpha-dilated outline +
    /// drop shadow, batched into the same instanced draw as buildings. `screen_pos` is
    /// the center in physical pixels and `half_size` the half-extent in pixels. Returns
    /// `false` (pushing nothing) if the emoji isn't in the atlas, so callers can fall
    /// back to the egui path.
    pub fn push_emoji(
        &mut self,
        emoji: &str,
        screen_pos: [f32; 2],
        half_size: f32,
        tint: [f32; 4],
        outline_color: [f32; 4],
    ) -> bool {
        let Some(icon_uv) = emoji_uv_opt(emoji) else {
            return false;
        };
        self.push_structure(StructureInstanceGpu {
            world_pos: screen_pos,
            size: half_size,
            shape_type: 4.0, // screen-space bare emoji
            color: tint,
            outline_color,
            icon_uv,
            opacity: 1.0,
        });
        true
    }

    /// Push a **world-space** bare emoji (no shape) with an alpha-dilated outline + drop
    /// shadow — used for map markers like buildings. `world_pos` is in world units and
    /// `half_size` is the half-extent in world units (the camera transform is applied in
    /// the shader). `opacity` dims the whole marker (e.g. under construction). Returns
    /// `false` if the emoji isn't in the atlas.
    pub fn push_world_emoji(
        &mut self,
        emoji: &str,
        world_pos: [f32; 2],
        half_size: f32,
        opacity: f32,
        outline_color: [f32; 4],
    ) -> bool {
        let Some(icon_uv) = emoji_uv_opt(emoji) else {
            return false;
        };
        self.push_structure(StructureInstanceGpu {
            world_pos,
            size: half_size,
            shape_type: 5.0, // world-space bare emoji
            color: [1.0, 1.0, 1.0, 1.0],
            outline_color,
            icon_uv,
            opacity,
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
            context.sync_buffer(self.buffer, 0, self.buffer.size());
        }
    }

    pub fn draw(
        &mut self,
        encoder: &mut gpu::CommandEncoder,
        target_view: gpu::TextureView,
        camera_pos: [f32; 2],
        zoom: f32,
        screen_size: [f32; 2],
        outline_px: f32,
        shadow_px: f32,
        context: &gpu::Context,
    ) {
        let instance_count = self.upload_instances.len() as u32;
        if instance_count == 0 {
            return;
        }

        self.write_buffers(context);

        let mut pass = encoder.render(
            "structure_pass",
            gpu::RenderTargetSet {
                colors: &[gpu::RenderTarget {
                    view: target_view,
                    init_op: gpu::InitOp::Load,
                    finish_op: gpu::FinishOp::Store,
                }],
                depth_stencil: None,
            },
        );

        let globals = StructureGlobals {
            camera_pos,
            zoom,
            _pad1: 0.0,
            screen_size,
            outline_px,
            shadow_px,
        };

        let shader_data = StructureShaderData {
            globals,
            icon_atlas: self.view,
            icon_sampler: self.sampler,
        };

        let mut rc = pass.with(&self.pipeline);
        rc.bind(0, &shader_data);
        rc.bind_vertex(0, self.buffer.at(0));
        rc.draw(0, 6, 0, instance_count);
    }

    pub fn destroy(&mut self, render_ctx: &RenderContext) {
        render_ctx.context.destroy_render_pipeline(&mut self.pipeline);
        render_ctx.context.destroy_buffer(self.buffer);
        render_ctx.context.destroy_sampler(self.sampler);
        render_ctx.context.destroy_texture_view(self.view);
        render_ctx.context.destroy_texture(self.texture);
        render_ctx.context.destroy_buffer(self.upload_buffer);
    }
}

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

pub fn emoji_uv(emoji: &str) -> [f32; 4] {
    emoji_uv_opt(emoji).unwrap_or([0.0, 0.0, 1.0, 1.0])
}
