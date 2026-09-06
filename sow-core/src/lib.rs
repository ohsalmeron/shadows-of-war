//! Shadows of War — Pure deterministic simulation core.
//!
//! This crate contains ALL game logic. It has zero knowledge of rendering,
//! networking, or platform. It compiles to `wasm32-unknown-unknown` with
//! zero platform dependencies.
//!
//! The single entry point is [`SowEngine::tick()`], which advances the
//! simulation by exactly one deterministic step.

pub mod bitset;
pub mod building;
pub mod config;
pub mod diplomacy;
#[cfg(test)]
mod diplomacy_engine_tests;
pub mod engine;
pub mod execution;
pub mod game;
pub mod game_config;
pub mod geo_entities;
pub mod intent;
pub mod map;
pub mod map_file;
pub mod pathfinding;
pub mod player;
pub mod rng;
pub mod sea_lane;
pub mod warp_fleet;

pub mod maps;
pub mod protocol;
pub mod tribes;
pub mod water_components;

pub use sow_data::commerce;

#[macro_export]
macro_rules! repo_asset_bytes {
    ($path:expr) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/", $path))
    };
}
pub use sow_data::{Civilization, Leader, NamedColor, PREMIUM_COLORS, leader_for_civilization};
