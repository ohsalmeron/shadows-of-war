use blade_graphics as gpu;
use crate::context::RenderContext;
use bytemuck::{Pod, Zeroable};
use sow_core::map::GameMap;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MapGlobals {
    pub camera_pos: [f32; 2],
    pub zoom: f32,
    pub time: f32,
    pub screen_size: [f32; 2],
    pub map_size: [f32; 2],
    pub visual_terrain_sharpness: f32,
    pub visual_interior_alpha: f32,
    pub visual_border_alpha: f32,
    pub visual_border_thickness: f32,
    pub effect_shockwave_intensity: f32,
    pub effect_border_breathe: f32,
    pub effect_energy_flow: f32,
    pub lod_2_zoom: f32,
    pub lod_3_zoom: f32,
    pub local_player_id: u32,
    /// Reserved for uniform layout; keep 0 (land opacity is rgb mix in `map.wgsl`).
    pub uniform_reserved: f32,
    /// WGSL uniform `struct Globals` is aligned to 8 (from `vec2` members), so its size must be a
    /// multiple of 8. Without this tail, `size_of::<MapGlobals>()` is 76 while Naga/SPIR-V use 80;
    /// Blade then binds a too-small UBO range, which breaks on stricter Vulkan (common on Android).
    pub padding2: u32,
}

#[derive(blade_macros::ShaderData)]
pub struct MapShaderData {
    globals: MapGlobals,
    territory_texture: gpu::TextureView,
    territory_sampler: gpu::Sampler,
    water_texture: gpu::TextureView,
    water_sampler: gpu::Sampler,
}



pub struct MapRenderer {
    pub texture: gpu::Texture,
    pub texture_view: gpu::TextureView,
    pub sampler: gpu::Sampler,
    pub water_texture: gpu::Texture,
    pub water_texture_view: gpu::TextureView,
    pub water_sampler: gpu::Sampler,
    pub pipeline: gpu::RenderPipeline,
    pub raw_buffer: gpu::Buffer,
    pub width: u32,
    pub height: u32,
    pub water_upload_buf: Option<gpu::Buffer>,
    pub prev_owners: Vec<u16>,
    pub conquest_flash: Vec<u8>,
    pub cached_pixels: Vec<u32>,
    pub dirty_flags: Vec<bool>,
    pub active_flashes: Vec<usize>,
}

impl MapRenderer {
    pub fn new(context: &gpu::Context, encoder: &mut gpu::CommandEncoder, width: u32, height: u32, surface_format: gpu::TextureFormat) -> Self {
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

        // Load and create the water texture
        let water_bytes = include_bytes!("../../sow-client/assets/water.bin");
        // Decode the simple R8 raw bytes (256x256 * 1 = 65536 bytes)
        let water_size = gpu::Extent { width: 256, height: 256, depth: 1 };
        
        let water_texture = context.create_texture(gpu::TextureDesc {
            name: "water_texture",
            format: gpu::TextureFormat::R8Unorm,
            size: water_size,
            dimension: gpu::TextureDimension::D2,
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            usage: gpu::TextureUsage::COPY | gpu::TextureUsage::RESOURCE,
            external: None,
        });

        let water_texture_view = context.create_texture_view(
            water_texture,
            gpu::TextureViewDesc {
                name: "water_texture_view",
                format: gpu::TextureFormat::R8Unorm,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );

        let water_sampler = context.create_sampler(gpu::SamplerDesc {
            name: "water_sampler",
            address_modes: [gpu::AddressMode::Repeat; 3], // Tiling!
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            mipmap_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });

        // Upload water pattern data
        let water_upload_buf = context.create_buffer(gpu::BufferDesc {
            name: "water_upload",
            size: water_bytes.len() as u64,
            memory: gpu::Memory::Shared,
        });
        unsafe {
            std::ptr::copy_nonoverlapping(
                water_bytes.as_ptr(),
                water_upload_buf.data(),
                water_bytes.len(),
            );
        }
        
        encoder.init_texture(water_texture);
        let mut transfer = encoder.transfer("water_upload_pass");
        transfer.copy_buffer_to_texture(
            water_upload_buf.into(),
            256, // 256 width * 1 bytes per pixel for R8Unorm
            water_texture.into(),
            gpu::Extent { width: 256, height: 256, depth: 1 },
        );
        drop(transfer);
        // We must keep water_upload_buf alive until the command encoder is submitted and executed.
        // So we will store it in the MapRenderer struct and clean it up later.

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
            "MapGlobals must match WGSL `struct Globals` uniform layout (see `padding2`)"
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

        Self {
            texture,
            texture_view,
            sampler,
            water_texture,
            water_texture_view,
            water_sampler,
            pipeline,
            raw_buffer,
            width,
            height,
            water_upload_buf: Some(water_upload_buf),
            prev_owners: vec![0; (width * height) as usize],
            conquest_flash: vec![0; (width * height) as usize],
            cached_pixels: Vec::new(),
            dirty_flags: Vec::new(),
            active_flashes: Vec::new(),
        }
    }

    /// Pack the game map into the upload buffer and copy to the GPU texture.
    pub fn update(&mut self, encoder: &mut gpu::CommandEncoder, context: &gpu::Context, map: &mut GameMap) {
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

        // 1. Scan for owner changes using map.dirty_tiles
        for i in map.dirty_tiles.drain(..) {
            if self.prev_owners[i] != map.state[i] {
                self.prev_owners[i] = map.state[i];
                let owner_id = map.state[i] as u32;
                if owner_id > 0 {
                    self.conquest_flash[i] = 255;
                    self.active_flashes.push(i);
                }
                
                // Mark center tile dirty
                if !self.dirty_flags[i] {
                    self.dirty_flags[i] = true;
                    dirty_indices.push(i);
                }
                
                // Mark neighbors dirty
                let y = (i as i32) / w;
                let x = (i as i32) % w;
                let is_odd = (y % 2) != 0;
                let neighbors_offsets = if is_odd {
                    [(1, 0), (-1, 0), (0, -1), (1, -1), (0, 1), (1, 1)]
                } else {
                    [(1, 0), (-1, 0), (-1, -1), (0, -1), (-1, 1), (0, 1)]
                };
                
                for (dx, dy) in neighbors_offsets.iter() {
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

        // 3. Process fading flashes
        let mut next_flashes = Vec::new();
        for &i in &self.active_flashes {
            if self.conquest_flash[i] > 0 {
                self.conquest_flash[i] = self.conquest_flash[i].saturating_sub(4);
                
                // Update flash byte directly in cache if not already marked dirty
                if !self.dirty_flags[i] {
                    let flash = self.conquest_flash[i] as u32;
                    self.cached_pixels[i] = (self.cached_pixels[i] & 0x00FFFFFF) | (flash << 24);
                }
                
                if self.conquest_flash[i] > 0 {
                    next_flashes.push(i);
                }
            }
        }
        self.active_flashes = next_flashes;

        if dirty_indices.is_empty() && self.active_flashes.is_empty() && !first_frame {
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
                let n_owner = map.state[ni] as u32;
                let n_t_byte = map.terrain[ni].as_byte() as u32;
                let n_is_water = (n_t_byte & 0x80) == 0;
                if c_is_water {
                    return !n_is_water;
                } else {
                    return (center_owner != n_owner) || (center_owner == 0 && n_is_water);
                }
            }
            !c_is_water
        };

        // 4. Update ONLY dirty tiles
        for i in dirty_indices {
            let y = (i as i32) / w;
            let x = (i as i32) % w;
            let terrain_byte = map.terrain[i].as_byte() as u32;
            let owner_id = map.state[i] as u32;
            let flash = self.conquest_flash[i] as u32;

            let c_is_water = (terrain_byte & 0x80) == 0;
            let mut border_mask = 0u32;
            let is_odd = (y % 2) != 0;
            
            if is_odd {
                if check_neighbor(x+1, y, owner_id, c_is_water) { border_mask |= 1; }
                if check_neighbor(x-1, y, owner_id, c_is_water) { border_mask |= 2; }
                if check_neighbor(x, y-1, owner_id, c_is_water) { border_mask |= 4; }
                if check_neighbor(x+1, y-1, owner_id, c_is_water) { border_mask |= 8; }
                if check_neighbor(x, y+1, owner_id, c_is_water) { border_mask |= 16; }
                if check_neighbor(x+1, y+1, owner_id, c_is_water) { border_mask |= 32; }
            } else {
                if check_neighbor(x+1, y, owner_id, c_is_water) { border_mask |= 1; }
                if check_neighbor(x-1, y, owner_id, c_is_water) { border_mask |= 2; }
                if check_neighbor(x-1, y-1, owner_id, c_is_water) { border_mask |= 4; }
                if check_neighbor(x, y-1, owner_id, c_is_water) { border_mask |= 8; }
                if check_neighbor(x-1, y+1, owner_id, c_is_water) { border_mask |= 16; }
                if check_neighbor(x, y+1, owner_id, c_is_water) { border_mask |= 32; }
            }

            self.cached_pixels[i] = owner_id | (border_mask << 10) | (terrain_byte << 16) | (flash << 24);
            self.dirty_flags[i] = false; // Reset dirty flag
        }

        // 5. Copy cached pixels to GPU mapped buffer
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
                water_texture: self.water_texture_view,
                water_sampler: self.water_sampler,
            },
        );
        rc.draw(0, 3, 0, 1);
    }

    pub fn destroy(&mut self, render_ctx: &RenderContext) {
        if let Some(buf) = self.water_upload_buf.take() {
            render_ctx.context.destroy_buffer(buf);
        }
        render_ctx.context.destroy_texture_view(self.water_texture_view);
        render_ctx.context.destroy_texture(self.water_texture);
        render_ctx.context.destroy_sampler(self.water_sampler);
        render_ctx.context.destroy_texture_view(self.texture_view);
        render_ctx.context.destroy_texture(self.texture);
        render_ctx.context.destroy_sampler(self.sampler);
        render_ctx.context.destroy_buffer(self.raw_buffer);
        render_ctx.context.destroy_render_pipeline(&mut self.pipeline);
    }
}
