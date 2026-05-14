use blade_graphics as gpu;
use crate::context::RenderContext;
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
    pub border_roundness: f32,
    pub _pad: [f32; 3],
}

#[derive(blade_macros::ShaderData)]
pub struct MapShaderData {
    globals: MapGlobals,
    territory_texture: gpu::TextureView,
}

pub struct MapRenderer {
    pub texture: gpu::Texture,
    pub texture_view: gpu::TextureView,
    pub pipeline: gpu::RenderPipeline,
    pub raw_buffer: gpu::Buffer,
    pub width: u32,
    pub height: u32,
    pub terrain: Vec<u8>,
    pub owners: Vec<u16>,
    pub bytes_per_row: u32,
}

impl MapRenderer {
    pub fn new(context: &gpu::Context, width: u32, height: u32, surface_format: gpu::TextureFormat, initial_terrain: &[u8]) -> Self {
        let bytes_per_row = (width * 4 + 255) & !255;
        let u32_per_row = bytes_per_row / 4;
        let total_u32 = (u32_per_row * height) as usize;

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

        let raw_buffer = context.create_buffer(gpu::BufferDesc {
            name: "map_raw",
            size: (bytes_per_row * height) as u64,
            memory: gpu::Memory::Upload,
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

        let dst_ptr = raw_buffer.data();
        let slice = unsafe {
            std::slice::from_raw_parts_mut(dst_ptr as *mut u32, total_u32)
        };
        for y in 0..height {
            for x in 0..width {
                let i = (y * width + x) as usize;
                let dst_i = (y * u32_per_row + x) as usize;
                slice[dst_i] = (initial_terrain[i] as u32) << 16;
            }
        }
        context.sync_buffer(raw_buffer);

        Self {
            texture,
            texture_view,
            pipeline,
            raw_buffer,
            width,
            height,
            terrain: initial_terrain.to_vec(),
            owners: vec![0; (width * height) as usize],
            bytes_per_row,
        }
    }

    /// Pack the game map into the upload buffer and copy to the GPU texture.
    pub fn update(&mut self, encoder: &mut blade_graphics::CommandEncoder, context: &blade_graphics::Context, snapshot: Option<&sow_core::protocol::SimSnapshot>) {
        let total = (self.width * self.height) as usize;
        let u32_per_row = self.bytes_per_row / 4;
        let total_u32 = (u32_per_row * self.height) as usize;
        
        let empty_tiles = vec![];
        let dirty_tiles = snapshot.map(|s| &s.dirty_tiles).unwrap_or(&empty_tiles);
        let is_defense_dirty = snapshot.map(|s| s.defense_dirty).unwrap_or(false);

        if dirty_tiles.is_empty() && !is_defense_dirty {
            return;
        }

        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0;
        let mut max_y = 0;

        let dst_ptr = self.raw_buffer.data();
        let slice = unsafe {
            std::slice::from_raw_parts_mut(dst_ptr as *mut u32, total_u32)
        };

        let empty_tiles = vec![];
        let dirty_tiles = snapshot.map(|s| &s.dirty_tiles).unwrap_or(&empty_tiles);
        let is_defense_dirty = snapshot.map(|s| s.defense_dirty).unwrap_or(false);

        for dt in dirty_tiles {
            let i = dt.index as usize;
            if i < total {
                self.owners[i] = dt.new_owner;
            }
        }

        if is_defense_dirty {
            for i in 0..total { self.terrain[i] &= !0x40; }
            let range = 8i32;
            if let Some(snap) = snapshot {
                for &post_idx in &snap.defense_posts {
                    let px = (post_idx % self.width) as i32;
                    let py = (post_idx / self.width) as i32;
                    for dy in -range..=range {
                        for dx in -range..=range {
                            if dx.abs() + dy.abs() <= range {
                                let nx = px + dx;
                                let ny = py + dy;
                                if nx >= 0 && ny >= 0 && nx < self.width as i32 && ny < self.height as i32 {
                                    let n_idx = (ny as u32 * self.width + nx as u32) as usize;
                                    self.terrain[n_idx] |= 0x40;
                                }
                            }
                        }
                    }
                }
            }
            min_x = 0; min_y = 0;
            max_x = self.width - 1; max_y = self.height - 1;
        }

        let pack_tile = |x: u32, y: u32, w: u32, h: u32, owners: &[u16], terrain: &[u8], slice: &mut [u32]| {
            let idx = (y * w + x) as usize;
            let owner_id = owners[idx] as u32;
            let terrain_byte = terrain[idx] as u32;
            
            let mut border_bits = 0u32;
            if owner_id > 0 {
                let up = if y > 0 { owners[idx - w as usize] as u32 } else { owner_id };
                let down = if y < h - 1 { owners[idx + w as usize] as u32 } else { owner_id };
                let left = if x > 0 { owners[idx - 1] as u32 } else { owner_id };
                let right = if x < w - 1 { owners[idx + 1] as u32 } else { owner_id };
                
                if owner_id != up { border_bits |= 1 << 31; }
                if owner_id != down { border_bits |= 1 << 30; }
                if owner_id != left { border_bits |= 1 << 29; }
                if owner_id != right { border_bits |= 1 << 28; }

                if (terrain_byte & 0x80) != 0 {
                    let up_ocean = if y > 0 { (terrain[idx - w as usize] & 0x80) == 0 } else { false };
                    let down_ocean = if y < h - 1 { (terrain[idx + w as usize] & 0x80) == 0 } else { false };
                    let left_ocean = if x > 0 { (terrain[idx - 1] & 0x80) == 0 } else { false };
                    let right_ocean = if x < w - 1 { (terrain[idx + 1] & 0x80) == 0 } else { false };
                    
                    if up_ocean { border_bits |= 1 << 27; }
                    if down_ocean { border_bits |= 1 << 26; }
                    if left_ocean { border_bits |= 1 << 25; }
                    if right_ocean { border_bits |= 1 << 24; }
                }
            }
            
            let dst_i = (y * u32_per_row + x) as usize;
            slice[dst_i] = (owner_id & 0xFFFF) | (terrain_byte << 16) | border_bits;
        };

        if is_defense_dirty {
            for y in 0..self.height {
                for x in 0..self.width {
                    pack_tile(x, y, self.width, self.height, &self.owners, &self.terrain, slice);
                }
            }
        } else {
            for dt in dirty_tiles {
                let i = dt.index as usize;
                if i >= total { continue; }
                let center_x = dt.index % self.width;
                let center_y = dt.index / self.width;
                
                let mut tiles_to_update = vec![(center_x, center_y)];
                if center_x > 0 { tiles_to_update.push((center_x - 1, center_y)); }
                if center_x < self.width - 1 { tiles_to_update.push((center_x + 1, center_y)); }
                if center_y > 0 { tiles_to_update.push((center_x, center_y - 1)); }
                if center_y < self.height - 1 { tiles_to_update.push((center_x, center_y + 1)); }

                for (x, y) in tiles_to_update {
                    pack_tile(x, y, self.width, self.height, &self.owners, &self.terrain, slice);
                    if x < min_x { min_x = x; }
                    if y < min_y { min_y = y; }
                    if x > max_x { max_x = x; }
                    if y > max_y { max_y = y; }
                }
            }
        }

        if min_x <= max_x && min_y <= max_y {
            let offset_bytes = (min_y * self.bytes_per_row + min_x * 4) as u64;
            let width_bytes = ((max_x - min_x + 1) * 4) as u64;
            let size_bytes = ((max_y - min_y) * self.bytes_per_row) as u64 + width_bytes;

            context.sync_buffer_range(self.raw_buffer, offset_bytes, size_bytes);

            let src_piece: gpu::BufferPiece = self.raw_buffer.at(offset_bytes);
            
            let mut dst_piece: gpu::TexturePiece = self.texture.into();
            dst_piece.origin = [min_x, min_y, 0];

            let mut transfer = encoder.transfer("map_upload");
            transfer.copy_buffer_to_texture(
                src_piece,
                self.bytes_per_row,
                dst_piece,
                gpu::Extent {
                    width: max_x - min_x + 1,
                    height: max_y - min_y + 1,
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
            },
        );
        rc.draw(0, 3, 0, 1);
    }

    pub fn destroy(&mut self, render_ctx: &RenderContext) {
        render_ctx.context.destroy_texture_view(self.texture_view);
        render_ctx.context.destroy_texture(self.texture);
        render_ctx.context.destroy_buffer(self.raw_buffer);
        render_ctx.context.destroy_render_pipeline(&mut self.pipeline);
    }
}
