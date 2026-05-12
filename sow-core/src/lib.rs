//! Shadows of War — Pure deterministic simulation core.
//!
//! This crate contains ALL game logic. It has zero knowledge of rendering,
//! networking, or platform. It compiles to `wasm32-unknown-unknown` with
//! zero platform dependencies.
//!
//! The single entry point is [`SowEngine::tick()`], which advances the
//! simulation by exactly one deterministic step.

pub mod map;
pub mod player;
pub mod game;
pub mod engine;
pub mod config;
pub mod game_config;
pub mod execution;
pub mod warp_fleet;
pub mod building;
pub mod intent;
pub mod pathfinding;
pub mod bitset;
pub mod rng;
pub mod checksum;

pub mod water_components;
pub mod protocol;
pub mod map_openfront;
pub mod ui_font;
