//! OpenFront-style weighted map playlists (mirrors `MapPlaylist.ts`).
//!
//! Each game mode (FFA, Teams, HumansVsNations) owns a deck built from
//! `multiplayer_frequency`
//! tickets per map — a map with frequency 20 appears 20x in the deck. The deck
//! is shuffled once, then consumed sequentially; a map is never drawn twice
//! within the last `NON_CONSECUTIVE` picks. When the deck is exhausted it is
//! rebuilt. Frequency 0 keeps a map out of the rotation entirely.
//!
//! Lobby capacity is derived per map from `num_land_tiles` (OpenFront formula):
//! 50 players per 1M land tiles, three tiers (100% / 75% / 50%), a 30/30/40
//! weighted roll, ×1.5 for Teams, capped at `MAX_PLAYER_CAP`.
//!
//! MENTAL MODEL (organic): the weighted shuffle is DELIBERATE randomness —
//! the same map must never show up on a predictable cadence, and every deck
//! rebuild reshuffles. Do not "fix" the randomness into a fixed order or a
//! deterministic cycle: predictable rotation is a regression.

use rand::Rng;
use sow_core::map_file::{MapCatalogEntry, MAX_PLAYER_CAP};
use std::sync::{Mutex, OnceLock};

/// No map repeats within the last N drawn picks (OpenFront `nonConsecutiveNum`).
pub const NON_CONSECUTIVE: usize = 5;

pub struct MapPlaylist {
    mode: &'static str,
    deck: Vec<String>,
    drawn: Vec<String>,
}

static FFA_PLAYLIST: OnceLock<Mutex<MapPlaylist>> = OnceLock::new();
static TEAMS_PLAYLIST: OnceLock<Mutex<MapPlaylist>> = OnceLock::new();
static HVN_PLAYLIST: OnceLock<Mutex<MapPlaylist>> = OnceLock::new();

fn playlist_for(mode: &str) -> &'static Mutex<MapPlaylist> {
    if mode == "Teams" {
        TEAMS_PLAYLIST.get_or_init(|| Mutex::new(MapPlaylist::new("Teams")))
    } else if mode == "HumansVsNations" {
        HVN_PLAYLIST.get_or_init(|| Mutex::new(MapPlaylist::new("HumansVsNations")))
    } else {
        FFA_PLAYLIST.get_or_init(|| Mutex::new(MapPlaylist::new("FFA")))
    }
}

impl MapPlaylist {
    fn new(mode: &'static str) -> Self {
        Self {
            mode,
            deck: Vec::new(),
            drawn: Vec::new(),
        }
    }

    /// Build the weighted deck: `multiplayer_frequency` copies per map, shuffled.
    /// The no-repeat window survives the rebuild, so a map never repeats within
    /// the last 5 picks even across deck boundaries.
    fn rebuild(&mut self, catalog: &[MapCatalogEntry]) {
        use rand::seq::SliceRandom;
        let mut maps = Vec::new();
        for entry in catalog {
            for _ in 0..entry.multiplayer_frequency {
                maps.push(entry.key.clone());
            }
        }
        maps.shuffle(&mut rand::thread_rng());
        self.deck = maps;
        log::info!(
            "[PLAYLIST] {} deck rebuilt: {} tickets across {} maps",
            self.mode,
            self.deck.len(),
            catalog.len()
        );
    }

    /// Draw the next map key: sequential deck consumption, skipping keys in the
    /// no-repeat window or no longer present in the catalog. Falls back to any
    /// remaining valid key when the whole deck is inside the window.
    fn next_map(&mut self, catalog: &[MapCatalogEntry]) -> Option<String> {
        if self.deck.is_empty() {
            self.rebuild(catalog);
        }
        if self.deck.is_empty() {
            return None;
        }
        for i in 0..self.deck.len() {
            let key = &self.deck[i];
            if !self.drawn.contains(key) && catalog.iter().any(|e| &e.key == key) {
                return Some(self.take(i));
            }
        }
        // Every remaining key is inside the window (or stale): pop the front anyway.
        let i = self
            .deck
            .iter()
            .position(|k| catalog.iter().any(|e| &e.key == k))
            .unwrap_or(0);
        if i < self.deck.len() {
            return Some(self.take(i));
        }
        self.deck.clear();
        None
    }

    fn take(&mut self, index: usize) -> String {
        let key = self.deck.remove(index);
        self.drawn.push(key.clone());
        if self.drawn.len() > NON_CONSECUTIVE {
            self.drawn.remove(0);
        }
        key
    }
}

/// Thread-safe draw of the next map key for a game mode.
pub fn next_map_for_mode(mode: &str, catalog: &[MapCatalogEntry]) -> Option<String> {
    match playlist_for(mode).lock() {
        Ok(mut playlist) => playlist.next_map(catalog),
        Err(_) => None,
    }
}

/// OpenFront lobby capacity derived from land tiles (MapPlaylist.ts
/// `calculateMapPlayerCounts` + `lobbyMaxPlayers`): `base = round5(lt/1M × 50)`
/// (min 5), tiers `[base, round5(base×0.75), round5(base×0.5)]`, weighted roll
/// 30% large / 30% mid / 40% small, Teams ceil(×1.5) capped at large, absolute
/// cap `MAX_PLAYER_CAP`, then rounded down to a multiple of 2 (Red/Blue teams).
pub fn lobby_max_players(entry: &MapCatalogEntry, game_mode: &str, rng: &mut impl Rng) -> u32 {
    if entry.num_land_tiles == 0 {
        // Unknown capacity (stale v1 catalog) — fall back to the global cap.
        return MAX_PLAYER_CAP;
    }
    let round5 = |n: f64| (n / 5.0).round() as u32 * 5;
    let base = round5(entry.num_land_tiles as f64 / 1_000_000.0 * 50.0).max(5);
    let large = base;
    let mid = round5(base as f64 * 0.75);
    let small = round5(base as f64 * 0.5);

    let roll = rng.gen_range(0.0..1.0);
    let mut players = if roll < 0.3 {
        large
    } else if roll < 0.6 {
        mid
    } else {
        small
    };

    if game_mode == "Teams" {
        players = ((players as f64) * 1.5).ceil() as u32;
        players = players.min(large);
        // Red/Blue teams only — keep the lobby even.
        players -= players % 2;
    }

    players.clamp(2, MAX_PLAYER_CAP)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn entry(key: &str, tiles: u32, freq: u32) -> MapCatalogEntry {
        MapCatalogEntry {
            key: key.to_string(),
            display_name: key.to_string(),
            width: 100,
            height: 100,
            num_land_tiles: tiles,
            multiplayer_frequency: freq,
        }
    }

    #[test]
    fn deck_respects_frequency_and_rotation() {
        let catalog = vec![entry("world", 651_569, 20), entry("oceania", 197_878, 0)];
        let mut playlist = MapPlaylist::new("FFA");
        let mut seen = std::collections::HashMap::new();
        for _ in 0..50 {
            let key = playlist.next_map(&catalog).unwrap();
            *seen.entry(key).or_insert(0) += 1;
        }
        assert!(seen.contains_key("world"));
        // frequency 0 → never drawn
        assert!(!seen.contains_key("oceania"));
    }

    #[test]
    fn no_repeat_within_window() {
        // Six maps so the 5-window constraint is always satisfiable.
        let catalog = vec![
            entry("a", 651_569, 1),
            entry("b", 651_569, 1),
            entry("c", 651_569, 1),
            entry("d", 651_569, 1),
            entry("e", 651_569, 1),
            entry("f", 651_569, 1),
        ];
        let mut playlist = MapPlaylist::new("FFA");
        let mut last: Vec<String> = Vec::new();
        for _ in 0..120 {
            let key = playlist.next_map(&catalog).unwrap();
            assert!(!last.contains(&key), "repeat within window: {key}");
            last.push(key);
            if last.len() > NON_CONSECUTIVE {
                last.remove(0);
            }
        }
    }

    #[test]
    fn capacity_follows_openfront_formula() {
        let mut rng = StdRng::seed_from_u64(7);
        // world: 651,569 tiles → base 35 → L/M/S = 35/25/20
        let world = entry("world", 651_569, 20);
        let mut caps = std::collections::HashSet::new();
        for _ in 0..200 {
            caps.insert(lobby_max_players(&world, "FFA", &mut rng));
        }
        for tier in [20u32, 25, 35] {
            assert!(caps.contains(&tier), "missing tier {tier} in {caps:?}");
        }
        // Teams ×1.5 rounded even, never above large (35 → up to 34)
        let mut team_caps = std::collections::HashSet::new();
        for _ in 0..200 {
            team_caps.insert(lobby_max_players(&world, "Teams", &mut rng));
        }
        for cap in &team_caps {
            assert!(*cap % 2 == 0, "odd team cap {cap}");
            assert!(*cap <= 35, "team cap {cap} above large tier");
        }
        // Unknown tiles → global cap
        let unknown = entry("x", 0, 1);
        assert_eq!(lobby_max_players(&unknown, "FFA", &mut rng), MAX_PLAYER_CAP);
    }
}
