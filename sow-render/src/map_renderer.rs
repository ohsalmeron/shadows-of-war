use blade_graphics as gpu;
use crate::context::RenderContext;
use bytemuck::{Pod, Zeroable};
use sow_core::map::GameMap;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MapGlobals {
    pub camera_pos: [f32; 2],
    pub zoom: f32,
    pub _pad0: f32,
    pub screen_size: [f32; 2],
    pub map_size: [f32; 2],
}

#[derive(blade_macros::ShaderData)]
pub struct MapShaderData {
    globals: MapGlobals,
    territory_texture: gpu::TextureView,
    territory_sampler: gpu::Sampler,
}

pub struct MapRenderer {
    pub texture: gpu::Texture,
    pub texture_view: gpu::TextureView,
    pub sampler: gpu::Sampler,
    pub pipeline: gpu::RenderPipeline,
    pub upload_buffer: gpu::Buffer,
    pub width: u32,
    pub height: u32,
}

impl MapRenderer {
    pub fn new(render_ctx: &RenderContext, width: u32, height: u32, surface_format: gpu::TextureFormat) -> Self {
        let texture = render_ctx.context.create_texture(gpu::TextureDesc {
            name: "territory_map",
            format: gpu::TextureFormat::R32Uint,
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

        let texture_view = render_ctx.context.create_texture_view(
            texture,
            gpu::TextureViewDesc {
                name: "territory_map_view",
                format: gpu::TextureFormat::R32Uint,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );

        let sampler = render_ctx.context.create_sampler(gpu::SamplerDesc {
            name: "map_sampler",
            mag_filter: gpu::FilterMode::Nearest,
            min_filter: gpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Upload buffer: 4 bytes per tile (u32), CPU-visible shared memory
        let buf_size = (width * height * 4) as u64;
        let upload_buffer = render_ctx.context.create_buffer(gpu::BufferDesc {
            name: "map_upload",
            size: buf_size,
            memory: gpu::Memory::Shared,
        });

        let shader_source = include_str!("shaders/map.wgsl");
        let shader = render_ctx.context.create_shader(gpu::ShaderDesc {
            source: shader_source,
            naga_module: None,
        });

        let layout = <MapShaderData as gpu::ShaderData>::layout();
        let pipeline = render_ctx.context.create_render_pipeline(gpu::RenderPipelineDesc {
            name: "map_pipeline",
            data_layouts: &[&layout],
            vertex: shader.at("vs_main"),
            vertex_fetches: &[],
            primitive: gpu::PrimitiveState {
                topology: gpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            fragment: Some(shader.at("fs_main")),
            color_targets: &[gpu::ColorTargetState {
                format: surface_format,
                blend: Some(gpu::BlendState::ALPHA_BLENDING),
                write_mask: gpu::ColorWrites::default(),
            }],
            multisample_state: gpu::MultisampleState::default(),
        });

        Self {
            texture,
            texture_view,
            sampler,
            pipeline,
            upload_buffer,
            width,
            height,
        }
    }

    /// Pack the game map into the upload buffer and copy to the GPU texture.
    /// Each texel is a u32: low 16 bits = owner_id, bits 16..24 = terrain byte.
    pub fn update(&self, encoder: &mut gpu::CommandEncoder, map: &GameMap) {
        let total = (self.width * self.height) as usize;
        let dst_ptr = self.upload_buffer.data();
        assert!(!dst_ptr.is_null(), "Upload buffer not mapped");

        // Write packed u32 per tile directly into the shared buffer
        let slice = unsafe {
            std::slice::from_raw_parts_mut(dst_ptr as *mut u32, total)
        };
        for i in 0..total {
            let terrain_byte = map.terrain[i].as_byte() as u32;
            let owner_id = map.state[i] as u32;
            // Pack: bits 0..15 = owner_id, bits 16..23 = terrain byte
            slice[i] = owner_id | (terrain_byte << 16);
        }

        // GPU transfer: copy upload buffer -> texture
        let bytes_per_row = self.width * 4; // 4 bytes per R32Uint texel
        {
            let mut transfer = encoder.transfer("map_upload");
            transfer.copy_buffer_to_texture(
                self.upload_buffer.into(),
                bytes_per_row,
                self.texture.into(),
                gpu::Extent {
                    width: self.width,
                    height: self.height,
                    depth: 1,
                },
            );
        }
    }

    pub fn draw(&self, encoder: &mut gpu::CommandEncoder, target_view: gpu::TextureView, globals: MapGlobals) {
        let mut pass = encoder.render(
            "map_pass",
            gpu::RenderTargetSet {
                colors: &[gpu::RenderTarget {
                    view: target_view,
                    init_op: gpu::InitOp::Clear(gpu::TextureColor::OpaqueBlack),
                    finish_op: gpu::FinishOp::Store,
                }],
                depth_stencil: None,
            },
        );
        let mut rc = pass.with(&self.pipeline);
        rc.bind(
            0,
            &MapShaderData {
                globals,
                territory_texture: self.texture_view,
                territory_sampler: self.sampler,
            },
        );
        rc.draw(0, 3, 0, 1);
    }

    pub fn destroy(&mut self, render_ctx: &RenderContext) {
        render_ctx.context.destroy_texture_view(self.texture_view);
        render_ctx.context.destroy_texture(self.texture);
        render_ctx.context.destroy_sampler(self.sampler);
        render_ctx.context.destroy_buffer(self.upload_buffer);
        render_ctx.context.destroy_render_pipeline(&mut self.pipeline);
    }
}
