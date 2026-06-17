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
    /// Up to 8 attack threat slots: [front_x, front_y, radius, packed_ids].
    pub threat_slots: [[f32; 4]; 8],
    pub effect_shockwave: f32,
    pub effect_breathe: f32,
    pub effect_energy_flow: f32,
    pub my_player_id: f32,
    pub hover_hex: [f32; 2],
    pub hover_building_kind: f32,
    pub territory_opacity: f32,
    /// Up to 8 fallout zones: [center_col, center_row, radius, alpha_progress].
    pub fallout_slots: [[f32; 4]; 8],
    /// Up to 32 nobuild exclusion zones: [center_col, center_row, radius, active].
    pub nobuild_slots: [[f32; 4]; 32],
    pub sub_voxel_scale: f32,
    pub blend_mode: f32,
    pub _pad3: f32,
    pub _pad4: f32,
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

fn get_neighbors(idx: u32, width: u32, height: u32) -> [Option<u32>; 4] {
    let x = idx % width;
    let y = idx / width;
    let deltas = [
        (1, 0),  // East
        (-1, 0), // West
        (0, -1), // North
        (0, 1),  // South
    ];
    let mut neighbors = [None; 4];
    for (i, &(dx, dy)) in deltas.iter().enumerate() {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
            neighbors[i] = Some((ny as u32) * width + (nx as u32));
        }
    }
    neighbors
}

fn compute_has_border(idx: u32, owners: &[u16], width: u32, height: u32) -> bool {
    let owner = owners[idx as usize];
    if owner == 0 {
        return false;
    }
    let neighbors = get_neighbors(idx, width, height);
    for &n_idx in &neighbors {
        if let Some(n) = n_idx {
            if owners[n as usize] != owner {
                return true;
            }
        } else {
            // Map edge has out-of-bounds neighbor (treated as owner 0)
            if owner != 0 {
                return true;
            }
        }
    }
    false
}

fn get_elevation_cpu(x: i32, y: i32, width: u32, height: u32, terrain: &[u8]) -> f32 {
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return 0.0;
    }
    let terrain_byte = terrain[(y as u32 * width + x as u32) as usize];
    let is_land = (terrain_byte & 0x80) != 0;
    if is_land {
        (terrain_byte & 0x1F) as f32
    } else {
        0.0
    }
}

fn compute_terrain_gradient(x: u32, y: u32, width: u32, height: u32, terrain: &[u8]) -> (f32, f32) {
    let cell_x = x as i32;
    let cell_y = y as i32;

    let h_right = get_elevation_cpu(cell_x + 1, cell_y, width, height, terrain);
    let h_left = get_elevation_cpu(cell_x - 1, cell_y, width, height, terrain);
    let h_up = get_elevation_cpu(cell_x, cell_y - 1, width, height, terrain);
    let h_down = get_elevation_cpu(cell_x, cell_y + 1, width, height, terrain);

    let dx = (h_right - h_left) * 0.10;
    let dy = (h_down - h_up) * 0.10;
    (dx, dy)
}

fn fill_terrain_buffer(
    terrain: &[u8],
    width: u32,
    height: u32,
    terrain_bytes_per_row: u32,
    terrain_slice: &mut [u8],
) {
    for y in 0..height {
        for x in 0..width {
            let src = (y * width + x) as usize;
            let dst = (y * terrain_bytes_per_row + x * 4) as usize;

            let terrain_byte = terrain[src];
            let (dx, dy) = compute_terrain_gradient(x, y, width, height, terrain);

            let packed_dx = (((dx + 8.0) / 16.0) * 255.0).round().clamp(0.0, 255.0) as u8;
            let packed_dy = (((dy + 8.0) / 16.0) * 255.0).round().clamp(0.0, 255.0) as u8;

            let seed = (x as u64)
                .wrapping_mul(374761393)
                .wrapping_add((y as u64).wrapping_mul(668265263));
            let hash = (seed ^ (seed >> 13)).wrapping_mul(1274126177);
            let noise_byte = (hash & 0xFF) as u8;

            terrain_slice[dst] = terrain_byte;
            terrain_slice[dst + 1] = packed_dx;
            terrain_slice[dst + 2] = packed_dy;
            terrain_slice[dst + 3] = noise_byte;
        }
    }
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
    pub conquest_flash: Vec<u8>,
    /// Tiles with non-zero flash, for sparse decay (avoids scanning all tiles).
    flash_active: Vec<u32>,
    pub terrain_bytes_per_row: u32,
    pub owner_bytes_per_row: u32,
    pub chunk_h: u32,
    pub dirty_chunks: Vec<bool>,
    pub last_update: Option<web_time::Instant>,
    pub has_water_neighbor: Vec<bool>,
    pub decay_accumulator: f32,
}

impl MapRenderer {
    pub fn new(
        context: &gpu::Context,
        width: u32,
        height: u32,
        surface_format: gpu::TextureFormat,
        initial_terrain: &[u8],
    ) -> Self {
        // Rgba8Unorm: 4 bytes per texel, row alignment to 256 bytes
        let terrain_bytes_per_row = (width * 4 + 255) & !255;
        // R32Uint: 4 bytes per texel, row alignment to 256 bytes
        let owner_bytes_per_row = (width * 4 + 255) & !255;

        // --- Terrain texture (Rgba8Unorm, static) ---
        let terrain_texture = context.create_texture(gpu::TextureDesc {
            name: "terrain_map",
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
        let terrain_view = context.create_texture_view(
            terrain_texture,
            gpu::TextureViewDesc {
                name: "terrain_map_view",
                format: gpu::TextureFormat::Rgba8Unorm,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );
        let terrain_buffer = context.create_buffer(gpu::BufferDesc {
            name: "terrain_raw",
            size: (terrain_bytes_per_row * height) as u64,
            memory: gpu::Memory::Upload,
        });

        // Compute static has_water_neighbor list
        let total = (width * height) as usize;
        let mut has_water_neighbor = vec![false; total];
        for idx in 0..total as u32 {
            let terrain_byte = initial_terrain[idx as usize];
            let is_land = (terrain_byte & 0x80) != 0;
            if is_land {
                let neighbors = get_neighbors(idx, width, height);
                let mut water = false;
                for &n_opt in &neighbors {
                    if let Some(n) = n_opt {
                        let n_terrain = initial_terrain[n as usize];
                        let n_is_land = (n_terrain & 0x80) != 0;
                        if !n_is_land {
                            water = true;
                            break;
                        }
                    } else {
                        // Map edge has out-of-bounds neighbor (treated as water/non-land)
                        water = true;
                        break;
                    }
                }
                has_water_neighbor[idx as usize] = water;
            }
        }

        // Fill terrain buffer with RGBA8 terrain bytes (R: terrain, G: normal dx, B: normal dy, A: CPU noise seed)
        let terrain_total = (terrain_bytes_per_row * height) as usize;
        let terrain_ptr = terrain_buffer.data();
        let terrain_slice = unsafe { std::slice::from_raw_parts_mut(terrain_ptr, terrain_total) };
        fill_terrain_buffer(
            initial_terrain,
            width,
            height,
            terrain_bytes_per_row,
            terrain_slice,
        );
        context.sync_buffer(terrain_buffer, 0, terrain_buffer.size());

        // --- Owner texture (R32Uint, dynamic) ---
        // Bits 0..15 = owner_id, bits 16..23 = conquest flash
        let owner_texture = context.create_texture(gpu::TextureDesc {
            name: "owner_map",
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
        let owner_view = context.create_texture_view(
            owner_texture,
            gpu::TextureViewDesc {
                name: "owner_map_view",
                format: gpu::TextureFormat::R32Uint,
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
        // Fragment entry contract: Linear swapchains use `fs_main`; plain UNORM (wasm WebGL
        // canvas) uses `fs_main_srgb`. Match on surface format like blade-egui.
        let fragment_entry = if matches!(surface_format, gpu::TextureFormat::Rgba8Unorm) {
            "fs_main_srgb"
        } else {
            "fs_main"
        };
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
            fragment: Some(shader.at(fragment_entry)),
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
            owners: vec![0; total],
            conquest_flash: vec![0; total],
            flash_active: Vec::new(),
            terrain_bytes_per_row,
            owner_bytes_per_row,
            chunk_h,
            dirty_chunks: vec![false; num_chunks as usize],
            last_update: None,
            has_water_neighbor,
            decay_accumulator: 0.0,
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

    /// Rebuild the terrain upload buffer from `self.terrain` and push it to the GPU.
    /// Used by the map editor when brush strokes change terrain bytes.
    pub fn sync_terrain_to_gpu(
        &mut self,
        encoder: &mut gpu::CommandEncoder,
        context: &gpu::Context,
    ) {
        let terrain_total = (self.terrain_bytes_per_row * self.height) as usize;
        let terrain_ptr = self.terrain_buffer.data();
        let terrain_slice = unsafe { std::slice::from_raw_parts_mut(terrain_ptr, terrain_total) };
        fill_terrain_buffer(
            &self.terrain,
            self.width,
            self.height,
            self.terrain_bytes_per_row,
            terrain_slice,
        );
        context.sync_buffer(self.terrain_buffer, 0, self.terrain_buffer.size());
        self.upload_terrain(encoder);
    }

    /// Pack and upload the full owner texture (shore/border/water-neighbor bits).
    pub fn upload_initial_owners(
        &mut self,
        encoder: &mut gpu::CommandEncoder,
        context: &gpu::Context,
    ) {
        let total = (self.width * self.height) as usize;
        let u32_per_row = self.owner_bytes_per_row / 4;
        let width = self.width;
        let height = self.height;

        let dst_ptr = self.owner_buffer.data();
        let slice = unsafe {
            std::slice::from_raw_parts_mut(dst_ptr as *mut u32, (u32_per_row * height) as usize)
        };

        for tile_idx in 0..total as u32 {
            let i = tile_idx as usize;
            let x = tile_idx % width;
            let y = tile_idx / width;
            let dst = (y * u32_per_row + x) as usize;

            let has_border = compute_has_border(tile_idx, &self.owners, width, height);
            let mut val = self.owners[i] as u32 | ((self.conquest_flash[i] as u32) << 16);
            if has_border {
                val |= 1 << 24;
            }
            if self.has_water_neighbor[i] {
                val |= 1 << 25;
            }
            slice[dst] = val;
        }

        context.sync_buffer(self.owner_buffer, 0, self.owner_buffer.size());

        let src_piece: gpu::BufferPiece = self.owner_buffer.into();
        let dst_piece: gpu::TexturePiece = self.owner_texture.into();
        let mut transfer = encoder.transfer("owner_initial_upload");
        transfer.copy_buffer_to_texture(
            src_piece,
            self.owner_bytes_per_row,
            dst_piece,
            gpu::Extent {
                width: self.width,
                height: self.height,
                depth: 1,
            },
        );
    }

    /// Write dirty ownership tiles to the upload buffer and copy to GPU.
    #[allow(unused_variables)]
    pub fn update(
        &mut self,
        encoder: &mut gpu::CommandEncoder,
        context: &gpu::Context,
        dirty_tiles: &[sow_core::protocol::DirtyTile],
        conquest_duration: f32,
    ) {
        let now = web_time::Instant::now();
        let dt = match self.last_update {
            Some(last) => now.duration_since(last).as_secs_f32(),
            None => 0.016,
        };
        self.last_update = Some(now);

        // Scale decay based on elapsed time so it completes in exactly `conquest_duration` seconds on all frame rates.
        // We use an accumulator to handle slow decay rates (longer lifetimes) smoothly on high frame rates,
        // avoiding the `max(1)` clamping issue when decay_amount is less than 1.
        let decay_rate = if conquest_duration > 0.0 {
            255.0 / conquest_duration
        } else {
            85.0
        };
        self.decay_accumulator += dt * decay_rate;
        let decay_amount = self.decay_accumulator.floor() as u32;
        if decay_amount > 0 {
            self.decay_accumulator -= decay_amount as f32;
        }

        let total = (self.width * self.height) as usize;
        let u32_per_row = self.owner_bytes_per_row / 4;
        let total_u32 = (u32_per_row * self.height) as usize;
        let width = self.width;
        let height = self.height;
        let chunk_h = self.chunk_h;

        let dst_ptr = self.owner_buffer.data();
        let slice = unsafe { std::slice::from_raw_parts_mut(dst_ptr as *mut u32, total_u32) };

        // Helper: pack owner + flash + border/water flags into the GPU buffer
        let pack =
            |slice: &mut [u32], owners: &[u16], flash: &[u8], tile_idx: u32, has_water: bool| {
                let i = tile_idx as usize;
                let x = tile_idx % width;
                let y = tile_idx / width;
                let dst = (y * u32_per_row + x) as usize;

                let has_border = compute_has_border(tile_idx, owners, width, height);

                let mut val = owners[i] as u32 | ((flash[i] as u32) << 16);
                if has_border {
                    val |= 1 << 24;
                }
                if has_water {
                    val |= 1 << 25;
                }
                slice[dst] = val;
            };

        // 1. Reset chunk tracking
        self.dirty_chunks.fill(false);

        // 2. Decay existing flash (sparse — only active entries)
        let mut decay_dirty = false;
        let has_water = &self.has_water_neighbor;
        if decay_amount > 0 {
            self.flash_active.retain(|&tile_idx| {
                let i = tile_idx as usize;
                if i >= total {
                    return false;
                }
                let f = self.conquest_flash[i].saturating_sub(decay_amount.min(255) as u8);
                self.conquest_flash[i] = f;
                pack(
                    slice,
                    &self.owners,
                    &self.conquest_flash,
                    tile_idx,
                    has_water[i],
                );
                let y = tile_idx / width;
                self.dirty_chunks[(y / chunk_h) as usize] = true;
                decay_dirty = true;
                f > 0
            });
        }

        // 3. Apply new dirty tiles
        for dt in dirty_tiles {
            let i = dt.index as usize;
            if i >= total {
                continue;
            }
            let old_owner = self.owners[i];
            if old_owner != dt.new_owner {
                if dt.new_owner > 0 {
                    self.conquest_flash[i] = 255;
                    self.flash_active.push(dt.index);
                }
                self.owners[i] = dt.new_owner;

                pack(
                    slice,
                    &self.owners,
                    &self.conquest_flash,
                    dt.index,
                    self.has_water_neighbor[i],
                );
                let y = dt.index / width;
                self.dirty_chunks[(y / chunk_h) as usize] = true;

                // Also update and dirty all neighbors since their border status changes
                let neighbors = get_neighbors(dt.index, width, height);
                for &n_opt in &neighbors {
                    if let Some(n_idx) = n_opt {
                        pack(
                            slice,
                            &self.owners,
                            &self.conquest_flash,
                            n_idx,
                            self.has_water_neighbor[n_idx as usize],
                        );
                        let n_y = n_idx / width;
                        self.dirty_chunks[(n_y / chunk_h) as usize] = true;
                    }
                }
            }
        }

        if dirty_tiles.is_empty() && !decay_dirty {
            return;
        }

        // Upload dirty chunks
        let num_chunks = self.dirty_chunks.len();
        let mut start_chunk = None;

        let mut upload_range = |start: usize, end: usize| {
            let min_y = (start as u32) * self.chunk_h;
            let max_y = (((end as u32) + 1) * self.chunk_h - 1).min(self.height - 1);

            let offset_bytes = (min_y * self.owner_bytes_per_row) as u64;
            let size_bytes =
                ((max_y - min_y) * self.owner_bytes_per_row) as u64 + self.width as u64 * 4;

            context.sync_buffer(self.owner_buffer, offset_bytes, size_bytes);

            let src_piece: gpu::BufferPiece = self.owner_buffer.at(offset_bytes);
            let mut dst_piece: gpu::TexturePiece = self.owner_texture.into();
            dst_piece.origin = [0, min_y, 0];

            let mut transfer = encoder.transfer("owner_upload");
            transfer.copy_buffer_to_texture(
                src_piece,
                self.owner_bytes_per_row,
                dst_piece,
                gpu::Extent {
                    width: self.width,
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
