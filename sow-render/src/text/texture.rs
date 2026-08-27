use blade_graphics as gpu;

pub struct FontAtlasTexture {
    pub texture: gpu::Texture,
    pub view: gpu::TextureView,
    pub buffer: gpu::Buffer,
    pub width: u32,
    pub height: u32,
}

impl FontAtlasTexture {
    pub fn from_bytes(
        context: &gpu::Context,
        png_bytes: &[u8],
        name: &str,
        format: gpu::TextureFormat,
    ) -> Self {
        let img = image::load_from_memory(png_bytes)
            .expect("Failed to load atlas PNG")
            .to_rgba8();
        let (width, height) = img.dimensions();
        let bytes_per_row = width * 4;
        let total = (bytes_per_row * height) as usize;

        let buffer = context.create_buffer(gpu::BufferDesc {
            name: &format!("{}_upload_buffer", name),
            size: total as u64,
            memory: gpu::Memory::Upload,
        });
        let dst = buffer.data();
        let slice = unsafe { std::slice::from_raw_parts_mut(dst, total) };
        slice.copy_from_slice(&img);
        context.sync_buffer(buffer, 0, buffer.size());

        let texture = context.create_texture(gpu::TextureDesc {
            name: &format!("{}_texture", name),
            format,
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
                name: &format!("{}_view", name),
                format,
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

    pub fn new(context: &gpu::Context) -> Self {
        let png_bytes = include_bytes!("../../../assets/gameplay/fonts/msdf-atlas.png");
        // ponytail: font atlas contains signed distance values (linear data), emoji atlas contains sRGB colors
        Self::from_bytes(
            context,
            png_bytes,
            "font_atlas",
            gpu::TextureFormat::Rgba8Unorm,
        )
    }

    pub fn blank(
        context: &gpu::Context,
        width: u32,
        height: u32,
        name: &str,
        format: gpu::TextureFormat,
    ) -> Self {
        let total = (width * 4 * height) as usize;
        let buffer = context.create_buffer(gpu::BufferDesc {
            name: &format!("{}_upload_buffer", name),
            size: total as u64,
            memory: gpu::Memory::Upload,
        });
        let dst = buffer.data();
        let slice = unsafe { std::slice::from_raw_parts_mut(dst, total) };
        slice.fill(0);
        context.sync_buffer(buffer, 0, buffer.size());

        let texture = context.create_texture(gpu::TextureDesc {
            name: &format!("{}_texture", name),
            format,
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
                name: &format!("{}_view", name),
                format,
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
        context.sync_buffer(self.buffer, 0, self.buffer.size());
        let src: gpu::BufferPiece = self.buffer.into();
        let dst: gpu::TexturePiece = self.texture.into();
        let mut transfer = encoder.transfer("font_atlas_upload");
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
}
