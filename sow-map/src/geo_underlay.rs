//! Blade GPU renderer for OpenStreetMap slippy-map tiles during geography selection.

use crate::osm_tiles::{CachedTile, OsmTileCache, TileKey, TILE_SIZE};
use blade_graphics as gpu;
use bytemuck::{Pod, Zeroable};
use image::RgbaImage;
use sow_render::RenderContext;
use std::collections::HashMap;

const MAX_TILES_PER_FRAME: usize = 512;
/// Upper bound on resident OSM tile textures in VRAM while panning.
const MAX_GPU_TILES: usize = 256;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GeoGlobals {
    pub camera_pos: [f32; 2],
    pub zoom: f32,
    pub _pad0: f32,
    pub screen_size: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, blade_macros::Vertex)]
pub struct GeoTileInstance {
    pub world_pos: [f32; 2],
    pub size: f32,
    pub _pad1: f32,
}

#[derive(blade_macros::ShaderData)]
struct GeoTileShaderData {
    globals: GeoGlobals,
    tile_tex: gpu::TextureView,
    tile_sampler: gpu::Sampler,
}

struct GpuOsmTile {
    texture: gpu::Texture,
    view: gpu::TextureView,
    buffer: gpu::Buffer,
}

pub struct GeoUnderlayRenderer {
    pipeline: gpu::RenderPipeline,
    instance_buffer: gpu::Buffer,
    sampler: gpu::Sampler,
    gpu_tiles: HashMap<TileKey, GpuOsmTile>,
    /// Insertion order for LRU eviction when `gpu_tiles` exceeds [`MAX_GPU_TILES`].
    tile_lru: Vec<TileKey>,
}

impl GeoUnderlayRenderer {
    pub fn new(context: &gpu::Context, surface_format: gpu::TextureFormat) -> Self {
        let source = include_str!("shaders/geo_tile.wgsl");
        let shader = context.create_shader(gpu::ShaderDesc {
            source,
            naga_module: None,
        });
        assert_eq!(
            std::mem::size_of::<GeoGlobals>(),
            shader.get_struct_size("GeoGlobals") as usize,
        );

        let layout = <GeoTileShaderData as gpu::ShaderData>::layout();
        let vertex_layout = <GeoTileInstance as gpu::Vertex>::layout();

        let pipeline = context.create_render_pipeline(gpu::RenderPipelineDesc {
            name: "geo_tile_pipeline",
            data_layouts: &[&layout],
            vertex: shader.at("vs_main"),
            vertex_fetches: &[gpu::VertexFetchState {
                layout: &vertex_layout,
                instanced: true,
            }],
            primitive: gpu::PrimitiveState::default(),
            depth_stencil: None,
            fragment: Some(shader.at("fs_main")),
            color_targets: &[gpu::ColorTargetState {
                format: surface_format,
                blend: None,
                write_mask: gpu::ColorWrites::default(),
            }],
            multisample_state: gpu::MultisampleState::default(),
        });

        let instance_buffer = context.create_buffer(gpu::BufferDesc {
            name: "geo_tile_instance",
            size: (MAX_TILES_PER_FRAME * std::mem::size_of::<GeoTileInstance>()) as u64,
            memory: gpu::Memory::Upload,
        });

        let sampler = context.create_sampler(gpu::SamplerDesc {
            name: "geo_tile_sampler",
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            pipeline,
            instance_buffer,
            sampler,
            gpu_tiles: HashMap::new(),
            tile_lru: Vec::new(),
        }
    }

    fn destroy_gpu_tile(render_ctx: &RenderContext, tile: GpuOsmTile) {
        render_ctx.context.destroy_texture_view(tile.view);
        render_ctx.context.destroy_texture(tile.texture);
        render_ctx.context.destroy_buffer(tile.buffer);
    }

    fn touch_lru(&mut self, key: TileKey) {
        if let Some(pos) = self.tile_lru.iter().position(|k| *k == key) {
            self.tile_lru.remove(pos);
        }
        self.tile_lru.push(key);
    }

    fn evict_lru_until_under_cap(&mut self, render_ctx: &RenderContext) {
        while self.gpu_tiles.len() >= MAX_GPU_TILES {
            let Some(oldest) = self.tile_lru.first().copied() else {
                break;
            };
            self.tile_lru.remove(0);
            if let Some(tile) = self.gpu_tiles.remove(&oldest) {
                Self::destroy_gpu_tile(render_ctx, tile);
            }
        }
    }

    pub fn clear_tiles(&mut self, render_ctx: &RenderContext) {
        for tile in self.gpu_tiles.drain().map(|(_, t)| t) {
            Self::destroy_gpu_tile(render_ctx, tile);
        }
        self.tile_lru.clear();
    }

    fn ensure_gpu_tile(
        &mut self,
        render_ctx: &RenderContext,
        key: TileKey,
        img: &RgbaImage,
    ) {
        if self.gpu_tiles.contains_key(&key) {
            self.touch_lru(key);
            return;
        }
        self.evict_lru_until_under_cap(render_ctx);

        let context = &render_ctx.context;
        let w = img.width();
        let h = img.height();
        let row_bytes = row_bytes_for(w);
        let buffer = context.create_buffer(gpu::BufferDesc {
            name: "geo_tile_upload",
            size: (row_bytes * h) as u64,
            memory: gpu::Memory::Upload,
        });
        let texture = context.create_texture(gpu::TextureDesc {
            name: "geo_osm_tile",
            format: gpu::TextureFormat::Rgba8Unorm,
            size: gpu::Extent {
                width: w,
                height: h,
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
                name: "geo_osm_tile_view",
                format: gpu::TextureFormat::Rgba8Unorm,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );
        self.gpu_tiles.insert(
            key,
            GpuOsmTile {
                texture,
                view,
                buffer,
            },
        );
        self.tile_lru.push(key);
    }

    fn upload_tile_pixels(
        &mut self,
        context: &gpu::Context,
        key: TileKey,
        img: &RgbaImage,
    ) {
        let Some(gpu_tile) = self.gpu_tiles.get_mut(&key) else {
            return;
        };
        let w = img.width();
        let h = img.height();
        let row_bytes = row_bytes_for(w);
        let raw = img.as_raw();
        let ptr = gpu_tile.buffer.data();
        let slice =
            unsafe { std::slice::from_raw_parts_mut(ptr, (row_bytes * h) as usize) };
        for row in 0..h as usize {
            let src_start = row * w as usize * 4;
            let dst_start = row * row_bytes as usize;
            slice[dst_start..dst_start + w as usize * 4]
                .copy_from_slice(&raw[src_start..src_start + w as usize * 4]);
        }
        context.sync_buffer(gpu_tile.buffer);
    }

    pub fn sync_tiles(
        &mut self,
        cache: &mut OsmTileCache,
        keys: &[TileKey],
        render_ctx: &mut RenderContext,
    ) {
        cache.drain_messages();
        for &key in keys {
            cache.request(key);
        }
        for &key in keys {
            if let Some(CachedTile::Ready(img)) = cache.get(key).cloned() {
                self.ensure_gpu_tile(render_ctx, key, &img);
            }
        }

        let context = &render_ctx.context;
        let encoder = &mut render_ctx.command_encoder;
        for &key in keys {
            if let Some(CachedTile::Ready(img)) = cache.get(key).cloned() {
                self.upload_tile_pixels(context, key, &img);
                let gpu_tile = self.gpu_tiles.get(&key).expect("tile inserted");
                let src: gpu::BufferPiece = gpu_tile.buffer.into();
                let dst: gpu::TexturePiece = gpu_tile.texture.into();
                let mut transfer = encoder.transfer("geo_tile_upload");
                transfer.copy_buffer_to_texture(
                    src,
                    row_bytes_for(img.width()),
                    dst,
                    gpu::Extent {
                        width: img.width(),
                        height: img.height(),
                        depth: 1,
                    },
                );
            }
        }
    }

    pub fn draw(
        &self,
        encoder: &mut gpu::CommandEncoder,
        context: &gpu::Context,
        target_view: gpu::TextureView,
        globals: GeoGlobals,
        visible_keys: &[(TileKey, f32, f32)],
        init_op: gpu::InitOp,
    ) {
        if visible_keys.is_empty() {
            let _pass = encoder.render(
                "geo_clear",
                gpu::RenderTargetSet {
                    colors: &[gpu::RenderTarget {
                        view: target_view,
                        init_op,
                        finish_op: gpu::FinishOp::Store,
                    }],
                    depth_stencil: None,
                },
            );
            return;
        }

        let mut pass = encoder.render(
            "geo_tiles",
            gpu::RenderTargetSet {
                colors: &[gpu::RenderTarget {
                    view: target_view,
                    init_op,
                    finish_op: gpu::FinishOp::Store,
                }],
                depth_stencil: None,
            },
        );

        for &(key, wx, wy) in visible_keys {
            let Some(gpu_tile) = self.gpu_tiles.get(&key) else {
                continue;
            };
            let inst = GeoTileInstance {
                world_pos: [wx, wy],
                size: TILE_SIZE as f32,
                _pad1: 0.0,
            };
            let ptr = self.instance_buffer.data();
            unsafe {
                std::ptr::copy_nonoverlapping(
                    &inst as *const GeoTileInstance as *const u8,
                    ptr,
                    std::mem::size_of::<GeoTileInstance>(),
                );
            }
            context.sync_buffer_range(
                self.instance_buffer,
                0,
                std::mem::size_of::<GeoTileInstance>() as u64,
            );
            let mut rc = pass.with(&self.pipeline);
            rc.bind(
                0,
                &GeoTileShaderData {
                    globals,
                    tile_tex: gpu_tile.view,
                    tile_sampler: self.sampler,
                },
            );
            rc.bind_vertex(0, self.instance_buffer.at(0));
            rc.draw(0, 6, 0, 1);
            drop(rc);
        }
        drop(pass);
    }

    pub fn destroy(&mut self, render_ctx: &RenderContext) {
        self.clear_tiles(render_ctx);
        render_ctx
            .context
            .destroy_render_pipeline(&mut self.pipeline);
        render_ctx
            .context
            .destroy_buffer(self.instance_buffer);
        render_ctx.context.destroy_sampler(self.sampler);
    }
}

fn row_bytes_for(width: u32) -> u32 {
    (width * 4 + 255) & !255
}
