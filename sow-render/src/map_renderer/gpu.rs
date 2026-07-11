use super::cpu_prep::{
    compute_has_border, fill_terrain_buffer, get_neighbors, MapGlobals, PlayerColors,
};
use crate::context::RenderContext;
use blade_graphics as gpu;
use blade_macros::ShaderData;

#[derive(ShaderData)]
struct MapShaderData {
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
    pub conquest_flash: Vec<u8>,
    /// Tiles with non-zero flash, for sparse decay (avoids scanning all tiles).
    flash_active: Vec<u32>,
    pub vision_fade: Vec<u8>,
    fade_active: Vec<u32>,
    is_fading: Vec<bool>,
    pub terrain_bytes_per_row: u32,
    pub owner_bytes_per_row: u32,
    pub chunk_h: u32,
    pub dirty_chunks: Vec<bool>,
    pub last_update: Option<web_time::Instant>,
    pub has_water_neighbor: Vec<bool>,
    pub decay_accumulator: f32,
    pub fade_decay_accumulator: f32,
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
        let source = include_str!("../shaders/map.wgsl");
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
            vision_fade: vec![0; total],
            fade_active: Vec::new(),
            is_fading: vec![false; total],
            terrain_bytes_per_row,
            owner_bytes_per_row,
            chunk_h,
            dirty_chunks: vec![false; num_chunks as usize],
            last_update: None,
            has_water_neighbor,
            decay_accumulator: 0.0,
            fade_decay_accumulator: 0.0,
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
        explored: &sow_core::bitset::DenseBitSet,
        visible: &sow_core::bitset::DenseBitSet,
        force_full_upload: bool,
    ) {
        let now = web_time::Instant::now();
        let dt = match self.last_update {
            Some(last) => now.duration_since(last).as_secs_f32(),
            None => 0.016,
        };
        self.last_update = Some(now);

        // Scale decay based on elapsed time so it completes in exactly `conquest_duration` seconds on all frame rates.
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

        // Helper: pack owner + flash + border/water/fog flags into the GPU buffer
        let pack =
            |slice: &mut [u32], owners: &[u16], flash: &[u8], fade: u8, tile_idx: u32, has_water: bool| {
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
                let fade_6bit = (fade >> 2) as u32; // scale 0..255 to 0..63
                val |= fade_6bit << 26;
                slice[dst] = val;
            };

        let has_water = &self.has_water_neighbor;

        // Pre-apply dirty tiles to self.owners and set conquest flash at the start
        // so both the full upload path and decay updates use the correct owners.
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
            }
        }

        // 1. Reset chunk tracking
        self.dirty_chunks.fill(false);

        // 2. Update vision fade targets based on current visibility (only when visibility changes)
        let mut any_fade_changed = false;
        if force_full_upload || !dirty_tiles.is_empty() {
            for tile_idx in 0..total as u32 {
                let is_vis = visible.contains(tile_idx);
                let current_fade = self.vision_fade[tile_idx as usize];
                if is_vis {
                    if current_fade < 255 {
                        self.vision_fade[tile_idx as usize] = 255;
                        self.is_fading[tile_idx as usize] = false;
                        pack(
                            slice,
                            &self.owners,
                            &self.conquest_flash,
                            255,
                            tile_idx,
                            has_water[tile_idx as usize],
                        );
                        let y = tile_idx / width;
                        self.dirty_chunks[(y / chunk_h) as usize] = true;
                        any_fade_changed = true;
                    }
                } else {
                    if current_fade > 0 {
                        if !self.is_fading[tile_idx as usize] {
                            self.is_fading[tile_idx as usize] = true;
                            self.fade_active.push(tile_idx);
                        }
                    }
                }
            }
        }

        if force_full_upload {
            for tile_idx in 0..total as u32 {
                let fade = self.vision_fade[tile_idx as usize];
                pack(
                    slice,
                    &self.owners,
                    &self.conquest_flash,
                    fade,
                    tile_idx,
                    has_water[tile_idx as usize],
                );
            }
            context.sync_buffer(self.owner_buffer, 0, self.owner_buffer.size());
            let src_piece: gpu::BufferPiece = self.owner_buffer.into();
            let dst_piece: gpu::TexturePiece = self.owner_texture.into();
            let mut transfer = encoder.transfer("owner_upload_full");
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
            return;
        }

        // 3. Decay existing flash (sparse — only active entries)
        let mut decay_dirty = false;
        if decay_amount > 0 {
            self.flash_active.retain(|&tile_idx| {
                let i = tile_idx as usize;
                if i >= total {
                    return false;
                }
                let f = self.conquest_flash[i].saturating_sub(decay_amount.min(255) as u8);
                self.conquest_flash[i] = f;
                let fade = self.vision_fade[i];
                pack(
                    slice,
                    &self.owners,
                    &self.conquest_flash,
                    fade,
                    tile_idx,
                    has_water[i],
                );
                let y = tile_idx / width;
                self.dirty_chunks[(y / chunk_h) as usize] = true;
                decay_dirty = true;
                f > 0
            });
        }

        // 4. Decay vision active fades (smoothly over 1.5 seconds)
        let fade_decay_rate = 170.0;
        self.fade_decay_accumulator += dt * fade_decay_rate;
        let fade_decay_amount = self.fade_decay_accumulator.floor() as u32;
        if fade_decay_amount > 0 {
            self.fade_decay_accumulator -= fade_decay_amount as f32;
        }

        let mut fade_dirty = false;
        if fade_decay_amount > 0 && !self.fade_active.is_empty() {
            let has_water = &self.has_water_neighbor;
            let vision_fade = &mut self.vision_fade;
            let is_fading = &mut self.is_fading;
            let owners = &self.owners;
            let conquest_flash = &self.conquest_flash;
            let dirty_chunks = &mut self.dirty_chunks;

            self.fade_active.retain(|&tile_idx| {
                let i = tile_idx as usize;
                if i >= total {
                    return false;
                }
                if !is_fading[i] {
                    return false; // Remove immediately since it became visible again
                }
                let f = vision_fade[i].saturating_sub(fade_decay_amount.min(255) as u8);
                vision_fade[i] = f;
                pack(
                    slice,
                    owners,
                    conquest_flash,
                    f,
                    tile_idx,
                    has_water[i],
                );
                let y = tile_idx / width;
                dirty_chunks[(y / chunk_h) as usize] = true;
                fade_dirty = true;
                let keep = f > 0;
                if !keep {
                    is_fading[i] = false;
                }
                keep
            });
        }

        // 5. Pack new dirty tiles and their neighbors
        for dt in dirty_tiles {
            let i = dt.index as usize;
            if i >= total {
                continue;
            }
            let fade = self.vision_fade[i];
            pack(
                slice,
                &self.owners,
                &self.conquest_flash,
                fade,
                dt.index,
                self.has_water_neighbor[i],
            );
            let y = dt.index / width;
            self.dirty_chunks[(y / chunk_h) as usize] = true;

            // Also update and dirty all neighbors since their border status changes
            let neighbors = get_neighbors(dt.index, width, height);
            for &n_opt in &neighbors {
                if let Some(n_idx) = n_opt {
                    let n_fade = self.vision_fade[n_idx as usize];
                    pack(
                        slice,
                        &self.owners,
                        &self.conquest_flash,
                        n_fade,
                        n_idx,
                        self.has_water_neighbor[n_idx as usize],
                    );
                    let n_y = n_idx / width;
                    self.dirty_chunks[(n_y / chunk_h) as usize] = true;
                }
            }
        }

        if dirty_tiles.is_empty() && !decay_dirty && !fade_dirty && !any_fade_changed {
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
