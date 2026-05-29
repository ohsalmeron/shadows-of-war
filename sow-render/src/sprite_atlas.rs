use blade_graphics as gpu;

/// Fixed sprite slots for GPU-instanced movers (must match WGSL / client mapping).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MoverSpriteId {
    TransportShip = 0,
    TradeShip = 1,
    Warship = 2,
    AtomBomb = 3,
    HydrogenBomb = 4,
    Mirv = 5,
    SamMissile = 6,
}

impl MoverSpriteId {
    pub const COUNT: usize = 7;
    pub const ATLAS_CELL: u32 = 64;

    pub fn uv_rect(self) -> [f32; 4] {
        let i = self as u32;
        let cell = Self::ATLAS_CELL as f32;
        let cols = 4u32;
        let col = i % cols;
        let row = i / cols;
        let u0 = (col * Self::ATLAS_CELL) as f32 / (cols * Self::ATLAS_CELL) as f32;
        let v0 = (row * Self::ATLAS_CELL) as f32 / (2.0 * cell);
        let u1 = u0 + cell / (cols * Self::ATLAS_CELL) as f32;
        let v1 = v0 + cell / (2.0 * cell);
        [u0, v0, u1, v1]
    }
}

const SPRITE_FILES: &[(&str, &[u8])] = &[
    (
        "transport_ship.svg",
        include_bytes!("../../sow-client/assets/transport_ship.svg"),
    ),
    (
        "trade_ship.svg",
        include_bytes!("../../sow-client/assets/trade_ship.svg"),
    ),
    (
        "battleship.svg",
        include_bytes!("../../sow-client/assets/battleship.svg"),
    ),
    (
        "atombomb.png",
        include_bytes!("../../sow-client/assets/atombomb.png"),
    ),
    (
        "hydrogenbomb.png",
        include_bytes!("../../sow-client/assets/hydrogenbomb.png"),
    ),
    (
        "mirv.png",
        include_bytes!("../../sow-client/assets/mirv.png"),
    ),
    (
        "sam_missile.png",
        include_bytes!("../../sow-client/assets/sam_missile.png"),
    ),
];

fn rasterize_sprite(bytes: &[u8], is_svg: bool, size: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    if is_svg {
        let opt = usvg::Options::default();
        let tree = usvg::Tree::from_data(bytes, &opt).expect("svg parse");
        let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("pixmap");
        let sx = size as f32 / tree.size().width().max(1.0);
        let sy = size as f32 / tree.size().height().max(1.0);
        let scale = sx.min(sy);
        let tx = (size as f32 - tree.size().width() * scale) * 0.5;
        let ty = (size as f32 - tree.size().height() * scale) * 0.5;
        let transform = tiny_skia::Transform::from_translate(tx, ty).pre_scale(scale, scale);
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        rgba.copy_from_slice(pixmap.data());
    } else {
        let img = image::load_from_memory(bytes).expect("png decode");
        let img = img.resize_exact(size, size, image::imageops::FilterType::Triangle);
        rgba.copy_from_slice(img.to_rgba8().as_raw());
    }
    rgba
}

pub struct SpriteAtlas {
    pub texture: gpu::Texture,
    pub view: gpu::TextureView,
    pub buffer: gpu::Buffer,
    pub width: u32,
    pub height: u32,
}

impl SpriteAtlas {
    pub fn new(context: &gpu::Context) -> Self {
        let cell = MoverSpriteId::ATLAS_CELL;
        let cols = 4u32;
        let rows = 2u32;
        let width = cols * cell;
        let height = rows * cell;
        let bytes_per_row = width * 4;
        let total = (bytes_per_row * height) as usize;

        let buffer = context.create_buffer(gpu::BufferDesc {
            name: "mover_atlas_upload",
            size: total as u64,
            memory: gpu::Memory::Upload,
        });
        let dst = buffer.data();
        let slice = unsafe { std::slice::from_raw_parts_mut(dst, total) };
        slice.fill(0);

        for (i, (name, bytes)) in SPRITE_FILES.iter().enumerate() {
            let is_svg = name.ends_with(".svg");
            let rgba = rasterize_sprite(bytes, is_svg, cell);
            let col = (i as u32) % cols;
            let row = (i as u32) / cols;
            for y in 0..cell {
                for x in 0..cell {
                    let src = ((y * cell + x) * 4) as usize;
                    let dst_x = col * cell + x;
                    let dst_y = row * cell + y;
                    let dst_i = (dst_y * bytes_per_row + dst_x * 4) as usize;
                    slice[dst_i..dst_i + 4].copy_from_slice(&rgba[src..src + 4]);
                }
            }
        }
        context.sync_buffer(buffer);

        let texture = context.create_texture(gpu::TextureDesc {
            name: "mover_atlas",
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
                name: "mover_atlas_view",
                format: gpu::TextureFormat::Rgba8Unorm,
                dimension: gpu::ViewDimension::D2,
                subresources: &gpu::TextureSubresources::default(),
            },
        );

        Self {
            texture,
            view,
            buffer,
            width,
            height,
        }
    }

    pub fn upload(&self, encoder: &mut gpu::CommandEncoder, context: &gpu::Context) {
        let bytes_per_row = self.width * 4;
        context.sync_buffer(self.buffer);
        let src: gpu::BufferPiece = self.buffer.into();
        let dst: gpu::TexturePiece = self.texture.into();
        let mut transfer = encoder.transfer("mover_atlas_upload");
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

    pub fn destroy(&mut self, render_ctx: &crate::context::RenderContext) {
        render_ctx.context.destroy_texture_view(self.view);
        render_ctx.context.destroy_texture(self.texture);
        render_ctx.context.destroy_buffer(self.buffer);
    }
}
