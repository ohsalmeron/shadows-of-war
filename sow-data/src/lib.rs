//! Static game data, player database, and embedded REDB metadata.

pub mod colors;
pub mod emoji;
pub mod geo_entities;
pub mod leaders;
pub mod tribes;
#[cfg(feature = "server")]
pub mod db;
#[cfg(feature = "server")]
pub mod crazygames;
#[cfg(feature = "server")]
pub mod time_util;
#[cfg(feature = "server")]
pub mod metadata_db;

pub use colors::{NamedColor, PREMIUM_COLORS};
pub use emoji::{lookup, AtlasRect, ATLAS_HEIGHT, ATLAS_WIDTH};
pub use leaders::{leader_for_civilization, Civilization, Leader};
pub use tribes::{
    animal_for_id, animal_for_name, empire_emoji_for_id, empire_emoji_for_name, EMPIRE_EMOJIS,
    FALLBACK_TRIBES, HISTORICAL_CIVILIZATIONS, TRIBE_ANIMALS,
};
#[cfg(feature = "server")]
pub use db::{PlayerDb, PlayerAccount, PlayerProfile, LinkedIdentity, AccountSummary};
#[cfg(feature = "server")]
pub use metadata_db::{init_database, get_leader_by_name, get_geo_entity_by_name, LeaderRecord, GeoEntityRecord};
