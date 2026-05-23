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
pub mod checksum;
pub mod config;
pub mod engine;
pub mod execution;
pub mod game;
pub mod game_config;
pub mod intent;
pub mod map;
pub mod pathfinding;
pub mod player;
pub mod rng;
pub mod sea_lane;
pub mod warp_fleet;

pub mod map_legacy;
pub mod protocol;
pub mod water_components;
pub mod tribes;
