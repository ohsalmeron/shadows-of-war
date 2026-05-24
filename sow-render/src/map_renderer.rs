use crate::context::RenderContext;
use blade_graphics as gpu;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MapGlobals {
    pub camera_pos: [f32; 2],
    pub zoom: f32,
    pub time: f32,
    pub screen_size: [f32; 2],
    pub map_size: [f32; 2],
    pub border_thickness: f32,
    pub border_darkness: f32,
    pub shore_thickness: f32,
    pub shore_darkness: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct PlayerColors {
    pub colors: [[f32; 4]; 256],
}

#[derive(blade_macros::ShaderData)]
pub struct MapShaderData {
    globals: MapGlobals,
    player_colors: PlayerColors,
    terrain_texture: gpu::TextureView,
    owner_texture: gpu::TextureView,
}

pub struct MapRenderer {
    pub terrain_texture: gpu::Texture,
    pub terrain_view: gpu::TextureView,
    pub terrain_buffer: gpu::Buffer,
    pub owner_texture: gpu::Texture,
    pub owner_view: gpu::TextureView,
    pub owner_buffer: gpu::Buffer,
    pub pipeline: gpu::RenderPipeline,
    pub width: u32,
    pub height: u32,
    pub terrain: Vec<u8>,
    pub owners: Vec<u16>,
    pub terrain_bytes_per_row: u32,
    pub owner_bytes_per_row: u32,
    pub chunk_h: u32,
    pub dirty_chunks: Vec<bool>,
}

impl MapRenderer {
    pub fn new(
        context: &gpu::Context,
        width: u32,
        height: u32,
        surface_format: gpu::TextureFormat,
        initial_terrain: &[u8],
    ) -> Self {
        let terrain_bytes_per_row = (width + 255) & !255;
        let owner_bytes_per_row = (width * 2 + 255) & !255;

        // --- Terrain texture (R8Uint, static) ---
        let terrain_texture = context.create_texture(gpu::TextureDesc {
            name: "terrain_map",
            format: gpu::TextureFormat::R8Uint,
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
        let terrain_view = context.create_texture_view(
            terrain_texture,
            gpu::TextureViewDesc {
                name: "terrain_map_view",
                format: gpu::TextureFormat::R8Uint,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );
        let terrain_buffer = context.create_buffer(gpu::BufferDesc {
            name: "terrain_raw",
            size: (terrain_bytes_per_row * height) as u64,
            memory: gpu::Memory::Upload,
        });

        // Fill terrain buffer with u8 terrain bytes
        let terrain_total = (terrain_bytes_per_row * height) as usize;
        let terrain_ptr = terrain_buffer.data();
        let terrain_slice = unsafe { std::slice::from_raw_parts_mut(terrain_ptr, terrain_total) };
        for y in 0..height {
            for x in 0..width {
                let src = (y * width + x) as usize;
                let dst = (y * terrain_bytes_per_row + x) as usize;
                terrain_slice[dst] = initial_terrain[src];
            }
        }
        context.sync_buffer(terrain_buffer);

        // --- Owner texture (R16Uint, dynamic) ---
        let owner_texture = context.create_texture(gpu::TextureDesc {
            name: "owner_map",
            format: gpu::TextureFormat::R16Uint,
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
        let owner_view = context.create_texture_view(
            owner_texture,
            gpu::TextureViewDesc {
                name: "owner_map_view",
                format: gpu::TextureFormat::R16Uint,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );
        let owner_buffer = context.create_buffer(gpu::BufferDesc {
            name: "owner_raw",
            size: (owner_bytes_per_row * height) as u64,
            memory: gpu::Memory::Upload,
        });

        // --- Shader & pipeline ---
        let source = include_str!("shaders/map.wgsl");
        let shader = context.create_shader(gpu::ShaderDesc {
            source,
            naga_module: None,
        });
        assert_eq!(
            std::mem::size_of::<MapGlobals>(),
            shader.get_struct_size("Globals") as usize,
            "MapGlobals must match WGSL `struct Globals` uniform layout"
        );
        assert_eq!(
            std::mem::size_of::<PlayerColors>(),
            shader.get_struct_size("PlayerColors") as usize,
            "PlayerColors must match WGSL `struct PlayerColors` uniform layout"
        );

        let layout = <MapShaderData as gpu::ShaderData>::layout();
        let pipeline = context.create_render_pipeline(gpu::RenderPipelineDesc {
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

        let chunk_h = 64;
        let num_chunks = height.div_ceil(chunk_h);

        Self {
            terrain_texture,
            terrain_view,
            terrain_buffer,
            owner_texture,
            owner_view,
            owner_buffer,
            pipeline,
            width,
            height,
            terrain: initial_terrain.to_vec(),
            owners: vec![0; (width * height) as usize],
            terrain_bytes_per_row,
            owner_bytes_per_row,
            chunk_h,
            dirty_chunks: vec![false; num_chunks as usize],
        }
    }

    /// Upload static terrain texture once. Call after creating the command encoder.
    pub fn upload_terrain(&self, encoder: &mut gpu::CommandEncoder) {
        let src_piece: gpu::BufferPiece = self.terrain_buffer.into();
        let dst_piece: gpu::TexturePiece = self.terrain_texture.into();
        let mut transfer = encoder.transfer("terrain_upload");
        transfer.copy_buffer_to_texture(
            src_piece,
            self.terrain_bytes_per_row,
            dst_piece,
            gpu::Extent {
                width: self.width,
                height: self.height,
                depth: 1,
            },
        );
    }

    /// Write dirty ownership tiles to the upload buffer and copy to GPU.
    pub fn update(
        &mut self,
        encoder: &mut gpu::CommandEncoder,
        context: &gpu::Context,
        dirty_tiles: &[sow_core::protocol::DirtyTile],
    ) {
        let total = (self.width * self.height) as usize;
        let u16_per_row = self.owner_bytes_per_row / 2;
        let total_u16 = (u16_per_row * self.height) as usize;

        if dirty_tiles.is_empty() {
            return;
        }

        let dst_ptr = self.owner_buffer.data();
        let slice = unsafe { std::slice::from_raw_parts_mut(dst_ptr as *mut u16, total_u16) };

        self.dirty_chunks.fill(false);

        for dt in dirty_tiles {
            let i = dt.index as usize;
            if i >= total {
                continue;
            }
            self.owners[i] = dt.new_owner;

            let x = dt.index % self.width;
            let y = dt.index / self.width;

            let dst_i = (y * u16_per_row + x) as usize;
            slice[dst_i] = dt.new_owner;

            self.dirty_chunks[(y / self.chunk_h) as usize] = true;
        }

        let num_chunks = self.dirty_chunks.len();
        let mut start_chunk = None;

        let mut upload_range = |start: usize, end: usize| {
            let min_y = (start as u32) * self.chunk_h;
            let mut max_y = ((end as u32) + 1) * self.chunk_h - 1;
            if max_y >= self.height {
                max_y = self.height - 1;
            }
            let aligned_min_x = 0;
            let aligned_max_x = self.width - 1;

            let offset_bytes = (min_y * self.owner_bytes_per_row + aligned_min_x * 2) as u64;
            let width_bytes = ((aligned_max_x - aligned_min_x + 1) * 2) as u64;
            let size_bytes = ((max_y - min_y) * self.owner_bytes_per_row) as u64 + width_bytes;

            context.sync_buffer_range(self.owner_buffer, offset_bytes, size_bytes);

            let src_piece: gpu::BufferPiece = self.owner_buffer.at(offset_bytes);

            let mut dst_piece: gpu::TexturePiece = self.owner_texture.into();
            dst_piece.origin = [aligned_min_x, min_y, 0];

            let mut transfer = encoder.transfer("owner_upload");
            transfer.copy_buffer_to_texture(
                src_piece,
                self.owner_bytes_per_row,
                dst_piece,
                gpu::Extent {
                    width: aligned_max_x - aligned_min_x + 1,
                    height: max_y - min_y + 1,
                    depth: 1,
                },
            );
        };

        for i in 0..num_chunks {
            if self.dirty_chunks[i] {
                if start_chunk.is_none() {
                    start_chunk = Some(i);
                }
            } else if let Some(start) = start_chunk {
                upload_range(start, i - 1);
                start_chunk = None;
            }
        }
        if let Some(start) = start_chunk {
            upload_range(start, num_chunks - 1);
        }
    }

    pub fn draw(
        &self,
        encoder: &mut gpu::CommandEncoder,
        target_view: gpu::TextureView,
        globals: MapGlobals,
        player_colors: PlayerColors,
    ) {
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
                player_colors,
                terrain_texture: self.terrain_view,
                owner_texture: self.owner_view,
            },
        );
        rc.draw(0, 3, 0, 1);
    }

    pub fn destroy(&mut self, render_ctx: &RenderContext) {
        render_ctx.context.destroy_texture_view(self.terrain_view);
        render_ctx.context.destroy_texture(self.terrain_texture);
        render_ctx.context.destroy_buffer(self.terrain_buffer);
        render_ctx.context.destroy_texture_view(self.owner_view);
        render_ctx.context.destroy_texture(self.owner_texture);
        render_ctx.context.destroy_buffer(self.owner_buffer);
        render_ctx
            .context
            .destroy_render_pipeline(&mut self.pipeline);
    }
}
