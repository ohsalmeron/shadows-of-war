use blade_graphics as gpu;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct TextGlobals {
    pub screen_size: [f32; 2],
    pub _pad: [f32; 2],
}

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub struct TmpFontSettings {
    pub face_dilate: f32,
    pub outline_thickness: f32,
    pub underlay_offset_y: f32,
    pub underlay_softness: f32,
}

impl Default for TmpFontSettings {
    fn default() -> Self {
        Self {
            face_dilate: 0.0,
            outline_thickness: 0.8,
            underlay_offset_y: 1.5,
            underlay_softness: 0.0,
        }
    }
}

pub const KIND_GLYPH: f32 = 0.0;
pub const KIND_EMOJI: f32 = 1.0;
pub const KIND_DISC: f32 = 2.0;
pub const KIND_RING: f32 = 3.0;
pub const KIND_SPRITE: f32 = 4.0;

pub const AVATAR_CELL: u32 = 128;
pub const AVATAR_COLS: u32 = 4;
pub const AVATAR_ROWS: u32 = 4;
pub const AVATAR_SLOT_COUNT: usize = (AVATAR_COLS * AVATAR_ROWS) as usize;

pub fn avatar_slot_uv(slot: usize) -> [f32; 4] {
    let i = slot as u32;
    let col = i % AVATAR_COLS;
    let row = i / AVATAR_COLS;
    let aw = (AVATAR_COLS * AVATAR_CELL) as f32;
    let ah = (AVATAR_ROWS * AVATAR_CELL) as f32;
    let u0 = (col * AVATAR_CELL) as f32 / aw;
    let v0 = (row * AVATAR_CELL) as f32 / ah;
    [u0, v0, u0 + AVATAR_CELL as f32 / aw, v0 + AVATAR_CELL as f32 / ah]
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, blade_macros::Vertex)]
pub struct TextInstanceGpu {
    pub screen_pos: [f32; 2],
    pub size: [f32; 2],
    pub uv_rect: [f32; 4],
    pub color: [f32; 4],
    pub outline_color: [f32; 4],
    pub face_dilate: f32,
    pub outline_thickness: f32,
    pub underlay_offset_y: f32,
    pub underlay_softness: f32,
    pub kind: f32,
}

#[derive(blade_macros::ShaderData)]
pub struct TextShaderData {
    pub globals: TextGlobals,
    pub font_atlas: gpu::TextureView,
    pub font_sampler: gpu::Sampler,
    pub emoji_atlas: gpu::TextureView,
    pub emoji_sampler: gpu::Sampler,
    pub avatar_atlas: gpu::TextureView,
    pub avatar_sampler: gpu::Sampler,
}
