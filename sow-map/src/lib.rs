#![warn(dead_code, unused_variables, unused_imports)]

pub mod editor;
pub mod generator;
pub mod image_pipeline;
pub mod thumbnail;

#[cfg(not(target_arch = "wasm32"))]
pub mod osm_tiles;

pub use editor::MapEditorSession;
pub use generator::{generate_map, GeneratorArgs, MapResult};
pub use image_pipeline::{generate_from_rgba, mobile_safe_dims, ImagePipelineResult};
pub use thumbnail::{
    encode_square_thumbnail_webp, terrain_preview_image, write_square_thumbnail,
    write_square_thumbnail_from_pixels, THUMBNAIL_SIZE,
};
