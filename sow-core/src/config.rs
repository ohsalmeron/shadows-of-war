//! Game balance constants.

/// LegacyEngine parity uses much larger gold magnitudes (125_000, 1_000_000, ...).
/// Keep those ratios by scaling into Shadows of War's lighter economy.
pub const GOLD_SCALE: f64 = 1_000.0;

/// Radius of the initial spawn cluster (in tiles from center).
pub const SPAWN_RADIUS: u32 = 5;

/// Manhattan range from a tile for defense post priority bonus.
pub const DEFENSE_POST_RANGE: i32 = 8;

/// Extra attack frontier priority per combined DefensePost level near the defender tile
/// (higher `PrioritizedTile::priority` ⇒ later conquest).
pub const DEFENSE_POST_PRIORITY_PER_LEVEL: i64 = 4;

/// Feature gate for incomplete missile gameplay (Silo + SAM interception loop).
pub const ENABLE_MISSILE_STRUCTURES: bool = true;
