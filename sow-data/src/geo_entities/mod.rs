//! Historical geo-entity database: tribes, city-states, kingdoms, empires and
//! countries with real-world coordinates. Spawn logic projects these onto any
//! map that carries geographic bounds, so AI names match the map's geography.
//!
//! Authoring rules (enforced by tests):
//! - names unique across the whole database
//! - lat in [-90, 90], lon in [-180, 180] (approximate historical centroid)
//! - flag is an ISO-2 country code or "" for entities without one
//!
//! Iteration order of [`ALL_GEO_ENTITIES`] is part of the deterministic
//! lockstep simulation: append new entries, never reorder wholesale within a
//! release cycle unless every client ships the change together (same rule as
//! any other sim change).

mod africa;
mod americas;
mod asia;
mod europe;
mod oceania;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    Tribe,
    CityState,
    Kingdom,
    Empire,
    Country,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Era {
    Ancient,
    Classical,
    Medieval,
    EarlyModern,
    Modern,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    Europe,
    Africa,
    Asia,
    Americas,
    Oceania,
}

#[derive(Debug, Clone, Copy)]
pub struct GeoEntity {
    pub name: &'static str,
    pub kind: EntityKind,
    pub era: Era,
    pub region: Region,
    pub lat: f32,
    pub lon: f32,
    /// ISO-2 flag code, or "" for entities predating modern flags.
    pub flag: &'static str,
}

/// One-line entry constructor used by the per-continent data files.
macro_rules! geo_entity {
    ($region:ident: $name:literal, $kind:ident, $era:ident, $lat:expr, $lon:expr) => {
        $crate::geo_entities::geo_entity!($region: $name, $kind, $era, $lat, $lon, "")
    };
    ($region:ident: $name:literal, $kind:ident, $era:ident, $lat:expr, $lon:expr, $flag:literal) => {
        $crate::geo_entities::GeoEntity {
            name: $name,
            kind: $crate::geo_entities::EntityKind::$kind,
            era: $crate::geo_entities::Era::$era,
            region: $crate::geo_entities::Region::$region,
            lat: $lat,
            lon: $lon,
            flag: $flag,
        }
    };
}
pub(crate) use geo_entity;

/// All entities in fixed declaration order (determinism-critical).
pub static ALL_GEO_ENTITIES: &[&[GeoEntity]] = &[
    europe::EUROPE,
    africa::AFRICA,
    asia::ASIA,
    americas::AMERICAS,
    oceania::OCEANIA,
];

/// Iterate every entity in stable order.
pub fn all() -> impl Iterator<Item = &'static GeoEntity> {
    ALL_GEO_ENTITIES.iter().flat_map(|s| s.iter())
}

/// Flag code for a named entity, for nameplate use ("" and misses map to None).
pub fn flag_for_name(name: &str) -> Option<&'static str> {
    all()
        .find(|e| e.name == name)
        .map(|e| e.flag)
        .filter(|f| !f.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_valid_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for e in all() {
            assert!(!e.name.is_empty(), "empty entity name");
            assert!(
                seen.insert(e.name),
                "duplicate geo entity name: {}",
                e.name
            );
            assert!(
                (-90.0..=90.0).contains(&e.lat),
                "{}: lat {} out of range",
                e.name,
                e.lat
            );
            assert!(
                (-180.0..=180.0).contains(&e.lon),
                "{}: lon {} out of range",
                e.name,
                e.lon
            );
            assert!(
                e.flag.is_empty() || e.flag.len() == 2,
                "{}: flag must be ISO-2 or empty, got '{}'",
                e.name,
                e.flag
            );
        }
        assert!(seen.len() >= 250, "database shrank: {} entries", seen.len());
    }

    #[test]
    fn every_continent_has_all_kinds() {
        use EntityKind::*;
        for (slice, region) in [
            (europe::EUROPE, Region::Europe),
            (africa::AFRICA, Region::Africa),
            (asia::ASIA, Region::Asia),
            (americas::AMERICAS, Region::Americas),
            (oceania::OCEANIA, Region::Oceania),
        ] {
            assert!(slice.iter().all(|e| e.region == region));
            for kind in [Tribe, CityState, Kingdom, Empire, Country] {
                assert!(
                    slice.iter().any(|e| e.kind == kind),
                    "{region:?} has no {kind:?} entries"
                );
            }
        }
    }
}
