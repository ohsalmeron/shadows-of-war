use crate::context::RenderContext;
use crate::sprite_atlas::SpriteAtlas;
use blade_graphics as gpu;
use bytemuck::{Pod, Zeroable};

pub const MAX_SPRITE_INSTANCES: usize = 16_384;
pub const MAX_TRAIL_SEGMENTS: usize = 32_768;
const VERTS_PER_QUAD: u32 = 6;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MoverGlobals {
    pub camera_pos: [f32; 2],
    pub zoom: f32,
    pub sprite_count: u32,
    pub screen_size: [f32; 2],
    pub trail_count: u32,
    pub _pad: f32,
}

/// One mover sprite. Uploaded as a single instance into an instanced vertex
/// buffer (divisor = 1); the quad corners are generated in the vertex shader
/// from `@builtin(vertex_index)`. No storage buffers, so this runs on WebGL2 /
/// GLES 3.0 (old Android) as well as native Vulkan.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, blade_macros::Vertex)]
pub struct MoverInstanceGpu {
    pub world_pos: [f32; 2],
    pub size: f32,
    pub rotation: f32,
    pub color: [f32; 4],
    pub uv_rect: [f32; 4],
    pub height: f32,
}

/// One trail segment, expanded to a quad in the vertex shader. Instanced.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, blade_macros::Vertex)]
pub struct TrailSegmentGpu {
    pub p0: [f32; 2],
    pub p1: [f32; 2],
    pub width: f32,
    pub color: [f32; 4],
}

#[derive(blade_macros::ShaderData)]
struct MoverSpriteShaderData {
    globals: MoverGlobals,
    sprite_atlas: gpu::TextureView,
    sprite_sampler: gpu::Sampler,
}

#[derive(blade_macros::ShaderData)]
struct MoverTrailShaderData {
    globals: MoverGlobals,
}

pub struct MoverRenderer {
    pub atlas: SpriteAtlas,
    sprite_pipeline: gpu::RenderPipeline,
    trail_pipeline: gpu::RenderPipeline,
    sprite_buffer: gpu::Buffer,
    trail_buffer: gpu::Buffer,
    sampler: gpu::Sampler,
    sprite_upload: Vec<MoverInstanceGpu>,
    trail_upload: Vec<TrailSegmentGpu>,
}

impl MoverRenderer {
    pub fn new(context: &gpu::Context, surface_format: gpu::TextureFormat) -> Self {
        let atlas = SpriteAtlas::new(context);

        let sprite_source = include_str!("shaders/mover_sprites.wgsl");
        let sprite_shader = context.create_shader(gpu::ShaderDesc {
            source: sprite_source,
            naga_module: None,
        });
        assert_eq!(
            std::mem::size_of::<MoverGlobals>(),
            sprite_shader.get_struct_size("MoverGlobals") as usize,
        );

        let trail_source = include_str!("shaders/mover_trails.wgsl");
        let trail_shader = context.create_shader(gpu::ShaderDesc {
            source: trail_source,
            naga_module: None,
        });
        assert_eq!(
            std::mem::size_of::<MoverGlobals>(),
            trail_shader.get_struct_size("MoverGlobals") as usize,
        );

        let sprite_layout = <MoverSpriteShaderData as gpu::ShaderData>::layout();
        let trail_layout = <MoverTrailShaderData as gpu::ShaderData>::layout();
        let sprite_vertex_layout = <MoverInstanceGpu as gpu::Vertex>::layout();
        let trail_vertex_layout = <TrailSegmentGpu as gpu::Vertex>::layout();
        let blend = gpu::BlendState {
            color: gpu::BlendComponent {
                src_factor: gpu::BlendFactor::SrcAlpha,
                dst_factor: gpu::BlendFactor::OneMinusSrcAlpha,
                operation: gpu::BlendOperation::Add,
            },
            alpha: gpu::BlendComponent::OVER,
        };

        // Fragment entry contract: sRGB swapchains (native) use `fs_main`; plain
        // UNORM (wasm WebGL canvas) uses `fs_main_srgb` to encode linear->sRGB
        // manually, so emoji/trail colors match native. Same contract as map.wgsl.
        let fragment_entry = if matches!(surface_format, gpu::TextureFormat::Rgba8Unorm) {
            "fs_main_srgb"
        } else {
            "fs_main"
        };

        let sprite_pipeline = context.create_render_pipeline(gpu::RenderPipelineDesc {
            name: "mover_sprite_pipeline",
            data_layouts: &[&sprite_layout],
            vertex: sprite_shader.at("vs_main"),
            vertex_fetches: &[gpu::VertexFetchState {
                layout: &sprite_vertex_layout,
                instanced: true,
            }],
            primitive: gpu::PrimitiveState::default(),
            depth_stencil: None,
            fragment: Some(sprite_shader.at(fragment_entry)),
            color_targets: &[gpu::ColorTargetState {
                format: surface_format,
                blend: Some(blend),
                write_mask: gpu::ColorWrites::default(),
            }],
            multisample_state: gpu::MultisampleState::default(),
        });

        let trail_pipeline = context.create_render_pipeline(gpu::RenderPipelineDesc {
            name: "mover_trail_pipeline",
            data_layouts: &[&trail_layout],
            vertex: trail_shader.at("vs_main"),
            vertex_fetches: &[gpu::VertexFetchState {
                layout: &trail_vertex_layout,
                instanced: true,
            }],
            primitive: gpu::PrimitiveState::default(),
            depth_stencil: None,
            fragment: Some(trail_shader.at(fragment_entry)),
            color_targets: &[gpu::ColorTargetState {
                format: surface_format,
                blend: Some(blend),
                write_mask: gpu::ColorWrites::default(),
            }],
            multisample_state: gpu::MultisampleState::default(),
        });

        let sprite_buffer = context.create_buffer(gpu::BufferDesc {
            name: "mover_sprite_instances",
            size: (MAX_SPRITE_INSTANCES * std::mem::size_of::<MoverInstanceGpu>()) as u64,
            memory: gpu::Memory::Upload,
        });
        let trail_buffer = context.create_buffer(gpu::BufferDesc {
            name: "mover_trail_instances",
            size: (MAX_TRAIL_SEGMENTS * std::mem::size_of::<TrailSegmentGpu>()) as u64,
            memory: gpu::Memory::Upload,
        });
        let sampler = context.create_sampler(gpu::SamplerDesc {
            name: "mover_atlas_sampler",
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            atlas,
            sprite_pipeline,
            trail_pipeline,
            sprite_buffer,
            trail_buffer,
            sampler,
            sprite_upload: Vec::with_capacity(4096),
            trail_upload: Vec::with_capacity(8192),
        }
    }

    pub fn upload_atlas(&self, encoder: &mut gpu::CommandEncoder, context: &gpu::Context) {
        self.atlas.upload(encoder, context);
    }

    pub fn begin_frame(&mut self) {
        self.sprite_upload.clear();
        self.trail_upload.clear();
    }

    pub fn push_sprite(&mut self, inst: MoverInstanceGpu) {
        if self.sprite_upload.len() < MAX_SPRITE_INSTANCES {
            self.sprite_upload.push(inst);
        }
    }

    pub fn push_trail_segment(&mut self, seg: TrailSegmentGpu) {
        if self.trail_upload.len() < MAX_TRAIL_SEGMENTS {
            self.trail_upload.push(seg);
        }
    }

    fn write_buffers(&self, context: &gpu::Context) {
        if !self.sprite_upload.is_empty() {
            let bytes = bytemuck::cast_slice(&self.sprite_upload);
            let dst = self.sprite_buffer.data();
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
            }
            // ponytail: only sync active slice to avoid massive WASM/WebGL overhead
            context.sync_buffer(self.sprite_buffer, 0, bytes.len() as u64);
        }
        if !self.trail_upload.is_empty() {
            let bytes = bytemuck::cast_slice(&self.trail_upload);
            let dst = self.trail_buffer.data();
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
            }
            // ponytail: only sync active slice to avoid massive WASM/WebGL overhead
            context.sync_buffer(self.trail_buffer, 0, bytes.len() as u64);
        }
    }

    pub fn draw(
        &mut self,
        encoder: &mut gpu::CommandEncoder,
        target_view: gpu::TextureView,
        globals: MoverGlobals,
        context: &gpu::Context,
    ) {
        let sprite_count = self.sprite_upload.len() as u32;
        let trail_count = self.trail_upload.len() as u32;
        if sprite_count == 0 && trail_count == 0 {
            return;
        }

        self.write_buffers(context);

        let mut pass = encoder.render(
            "mover_pass",
            gpu::RenderTargetSet {
                colors: &[gpu::RenderTarget {
                    view: target_view,
                    init_op: gpu::InitOp::Load,
                    finish_op: gpu::FinishOp::Store,
                }],
                depth_stencil: None,
            },
        );

        let globals = MoverGlobals {
            sprite_count,
            trail_count,
            ..globals
        };

        if trail_count > 0 {
            let trail_data = MoverTrailShaderData { globals };
            let mut rc = pass.with(&self.trail_pipeline);
            rc.bind(0, &trail_data);
            rc.bind_vertex(0, self.trail_buffer.at(0));
            rc.draw(0, VERTS_PER_QUAD, 0, trail_count);
        }

        if sprite_count > 0 {
            let sprite_data = MoverSpriteShaderData {
                globals,
                sprite_atlas: self.atlas.view,
                sprite_sampler: self.sampler,
            };
            let mut rc = pass.with(&self.sprite_pipeline);
            rc.bind(0, &sprite_data);
            rc.bind_vertex(0, self.sprite_buffer.at(0));
            rc.draw(0, VERTS_PER_QUAD, 0, sprite_count);
        }
    }

    pub fn destroy(&mut self, render_ctx: &RenderContext) {
        render_ctx
            .context
            .destroy_render_pipeline(&mut self.sprite_pipeline);
        render_ctx
            .context
            .destroy_render_pipeline(&mut self.trail_pipeline);
        render_ctx.context.destroy_buffer(self.sprite_buffer);
        render_ctx.context.destroy_buffer(self.trail_buffer);
        render_ctx.context.destroy_sampler(self.sampler);
        self.atlas.destroy(render_ctx);
    }
}
