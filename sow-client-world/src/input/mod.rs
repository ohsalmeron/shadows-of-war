pub(crate) mod intents;
pub(crate) mod map_click;
pub(crate) mod placement;
pub(crate) mod surface;
pub(crate) mod window;

#[cfg(test)]
mod tests;

pub use placement::{
    find_stack_target_tile, resolve_build_target_tile, resolve_building_placement_tile,
};
