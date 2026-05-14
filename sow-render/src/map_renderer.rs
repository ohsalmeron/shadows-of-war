use blade_graphics as gpu;
use crate::context::RenderContext;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MapGlobals {
    pub camera_pos: [f32; 2],
    pub zoom: f32,
    pub _pad0: f32,
    pub screen_size: [f32; 2],
    pub map_size: [f32; 2],
    pub local_player_id: u32,
    pub _pad1: u32,
    pub _pad2: u32,
    pub _pad3: u32,
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
    pub raw_buffer: gpu::Buffer,
    pub width: u32,
    pub height: u32,
    pub prev_owners: Vec<u16>,
    pub cached_pixels: Vec<u32>,
    pub dirty_flags: Vec<bool>,
    pub terrain: Vec<u8>,
}

impl MapRenderer {
    pub fn new(context: &gpu::Context, encoder: &mut gpu::CommandEncoder, width: u32, height: u32, surface_format: gpu::TextureFormat, initial_terrain: &[u8]) -> Self {
        let texture = context.create_texture(gpu::TextureDesc {
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

        let texture_view = context.create_texture_view(
            texture,
            gpu::TextureViewDesc {
                name: "territory_map_view",
                format: gpu::TextureFormat::R32Uint,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );

        let sampler = context.create_sampler(gpu::SamplerDesc {
            name: "map_sampler",
            address_modes: [gpu::AddressMode::ClampToEdge; 3],
            mag_filter: gpu::FilterMode::Nearest,
            min_filter: gpu::FilterMode::Nearest,
            mipmap_filter: gpu::FilterMode::Nearest,
            ..Default::default()
        });

        let raw_buffer = context.create_buffer(gpu::BufferDesc {
            name: "map_raw",
            size: (width * height * 4) as u64,
            memory: gpu::Memory::Shared,
        });

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

        encoder.init_texture(texture);

        Self {
            texture,
            texture_view,
            sampler,
            pipeline,
            raw_buffer,
            width,
            height,
            prev_owners: vec![0; (width * height) as usize],
            cached_pixels: Vec::new(),
            dirty_flags: Vec::new(),
            terrain: initial_terrain.to_vec(),
        }
    }

    /// Pack the game map into the upload buffer and copy to the GPU texture.
    pub fn update(&mut self, encoder: &mut gpu::CommandEncoder, context: &gpu::Context, dirty_tiles: &[sow_core::protocol::DirtyTile]) {
        let total = (self.width * self.height) as usize;
        
        let mut first_frame = false;
        if self.cached_pixels.is_empty() {
            self.cached_pixels = vec![0; total];
            self.dirty_flags = vec![true; total];
            first_frame = true;
        }

        let mut dirty_indices = Vec::new();
        let w = self.width as i32;
        let h = self.height as i32;

        // 1. Scan for owner changes using dirty_tiles
        for dt in dirty_tiles {
            let i = dt.index as usize;
            if i >= self.prev_owners.len() {
                continue;
            }
            if self.prev_owners[i] != dt.new_owner {
                self.prev_owners[i] = dt.new_owner;
                
                // Mark center tile dirty
                if !self.dirty_flags[i] {
                    self.dirty_flags[i] = true;
                    dirty_indices.push(i);
                }
                
                // Mark neighbors dirty (4 cardinal)
                let y = (i as i32) / w;
                let x = (i as i32) % w;
                let neighbors_offsets = [(1, 0), (-1, 0), (0, -1), (0, 1)];
                
                for &(dx, dy) in neighbors_offsets.iter() {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx >= 0 && nx < w && ny >= 0 && ny < h {
                        let ni = (ny * w + nx) as usize;
                        if !self.dirty_flags[ni] {
                            self.dirty_flags[ni] = true;
                            dirty_indices.push(ni);
                        }
                    }
                }
            }
        }

        // 2. Add previously full-dirty pass tiles if first frame
        if first_frame {
            for i in 0..total {
                dirty_indices.push(i);
            }
        }

        if dirty_indices.is_empty() && !first_frame {
            return;
        }

        let dst_ptr = self.raw_buffer.data();
        assert!(!dst_ptr.is_null(), "Raw buffer not mapped");

        let slice = unsafe {
            std::slice::from_raw_parts_mut(dst_ptr as *mut u32, total)
        };
        
        let check_neighbor = |nx: i32, ny: i32, center_owner: u32, c_is_water: bool| -> bool {
            if nx >= 0 && ny >= 0 && nx < w && ny < h {
                let ni = (ny * w + nx) as usize;
                let n_owner = self.prev_owners[ni] as u32;
                let n_t_byte = self.terrain[ni] as u32;
                let n_is_water = (n_t_byte & 0x80) == 0;
                if c_is_water {
                    return !n_is_water;
                } else {
                    return (center_owner != n_owner) || (center_owner == 0 && n_is_water);
                }
            }
            !c_is_water
        };

        // 3. Update ONLY dirty tiles
        for i in dirty_indices {
            let y = (i as i32) / w;
            let x = (i as i32) % w;
            let terrain_byte = self.terrain[i] as u32;
            let owner_id = self.prev_owners[i] as u32;

            let c_is_water = (terrain_byte & 0x80) == 0;
            let mut border_mask = 0u32;
            
            // East
            if check_neighbor(x+1, y, owner_id, c_is_water) { border_mask |= 1; }
            // West
            if check_neighbor(x-1, y, owner_id, c_is_water) { border_mask |= 2; }
            // North
            if check_neighbor(x, y-1, owner_id, c_is_water) { border_mask |= 4; }
            // South
            if check_neighbor(x, y+1, owner_id, c_is_water) { border_mask |= 8; }

            self.cached_pixels[i] = (owner_id & 0xFFF) | (border_mask << 12) | (terrain_byte << 16);
            self.dirty_flags[i] = false; // Reset dirty flag
        }

        // 4. Copy cached pixels to GPU mapped buffer
        slice.copy_from_slice(&self.cached_pixels);

        context.sync_buffer(self.raw_buffer);

        // Copy baked buffer to texture
        let bytes_per_row = self.width * 4;
        {
            let mut transfer = encoder.transfer("map_upload");
            transfer.copy_buffer_to_texture(
                self.raw_buffer.into(),
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
        render_ctx.context.destroy_buffer(self.raw_buffer);
        render_ctx.context.destroy_render_pipeline(&mut self.pipeline);
    }
}
