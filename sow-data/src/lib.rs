//! Static game data, player database, and embedded REDB metadata.

pub mod colors;
#[cfg(feature = "server")]
pub mod crazygames;
#[cfg(feature = "server")]
pub mod db;
pub mod emoji;
pub mod geo_entities;
pub mod leaders;
#[cfg(feature = "server")]
pub mod metadata_db;
#[cfg(feature = "server")]
pub mod time_util;
pub mod tribes;

pub use colors::{NamedColor, PREMIUM_COLORS};
#[cfg(feature = "server")]
pub use db::{AccountSummary, LinkedIdentity, PlayerAccount, PlayerDb, PlayerProfile};
pub use emoji::{ATLAS_HEIGHT, ATLAS_WIDTH, AtlasRect, lookup};
pub use leaders::{Civilization, Leader, leader_for_civilization};
#[cfg(feature = "server")]
pub use metadata_db::{
    GeoEntityRecord, LeaderRecord, get_geo_entity_by_name, get_leader_by_name, init_database,
};
pub use tribes::{
    EMPIRE_EMOJIS, FALLBACK_TRIBES, HISTORICAL_CIVILIZATIONS, TRIBE_ANIMALS, animal_for_id,
    animal_for_name, empire_emoji_for_id, empire_emoji_for_name,
};
