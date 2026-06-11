#![warn(dead_code, unused_variables, unused_imports)]

#[cfg(feature = "editor")]
pub mod editor;

#[cfg(feature = "generator")]
pub mod generator;
#[cfg(feature = "generator")]
pub mod heightmap;
#[cfg(feature = "generator")]
pub mod image_pipeline;
#[cfg(feature = "generator")]
pub mod thumbnail;

#[cfg(feature = "osm")]
pub mod osm_coast;
#[cfg(feature = "osm")]
pub mod osm_overpass;
#[cfg(feature = "osm")]
pub mod osm_tiles;

#[cfg(target_arch = "wasm32")]
pub mod wasm_export;

#[cfg(feature = "editor")]
pub use editor::MapEditorSession;

#[cfg(feature = "generator")]
pub use generator::{generate_map, GeneratorArgs, MapResult};
#[cfg(feature = "generator")]
pub use image_pipeline::{generate_from_rgba, mobile_safe_dims, ImagePipelineResult};
#[cfg(feature = "generator")]
pub use thumbnail::{
    encode_square_thumbnail_webp, terrain_preview_image, write_square_thumbnail,
    write_square_thumbnail_from_pixels, THUMBNAIL_SIZE,
};
