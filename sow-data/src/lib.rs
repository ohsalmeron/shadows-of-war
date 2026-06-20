//! Static game data: color tables, tribe names, leader metadata, emoji atlas UVs.

pub mod colors;
pub mod emoji;
pub mod leaders;
pub mod tribes;

pub use colors::{NamedColor, PREMIUM_COLORS};
pub use emoji::{lookup, AtlasRect, ATLAS_HEIGHT, ATLAS_WIDTH};
pub use leaders::{leader_for_civilization, Civilization, Leader};
pub use tribes::{
    animal_for_id, animal_for_name, FALLBACK_TRIBES, HISTORICAL_CIVILIZATIONS, TRIBE_ANIMALS,
};
