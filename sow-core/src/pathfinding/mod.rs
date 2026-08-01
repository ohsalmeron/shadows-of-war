//! Deterministic water A* for fleet routing. No HashMap iteration; integer costs only.

mod astar;
mod flow_field;

pub use astar::{WaterAStar, WaterPathfinderScratch, bresenham_line};
pub use flow_field::{FlowField, FlowFieldCache};
