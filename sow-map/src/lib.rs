#![warn(dead_code, unused_variables, unused_imports)]

#[cfg(feature = "editor")]
extern crate sow_ui as sow_ui_kit;
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
pub use generator::{GeneratorArgs, MapResult, generate_map};
#[cfg(feature = "generator")]
pub use image_pipeline::{ImagePipelineResult, generate_from_rgba, mobile_safe_dims};
#[cfg(feature = "generator")]
pub use thumbnail::{
    SourceFrame, THUMBNAIL_HEIGHT, THUMBNAIL_SIZE, THUMBNAIL_WIDTH,
    encode_square_thumbnail_webp, terrain_preview_image, write_map_thumbnail,
    render_source_file, render_source_image, write_rendered_source_thumbnail,
    write_source_thumbnail, write_square_thumbnail, write_square_thumbnail_from_pixels,
    write_wide_thumbnail,
};
