//! Campaign scripting. An *episode* places a fixed roster of named, teamed, colored bots on a
//! map; the engine consumes it as `GameConfig.scripted_spawns` (see `sow_core::game_config::
//! ScriptedSpawn`). Adding an episode = a new submodule returning `Vec<Faction>` + the player's
//! homeland tile. A faction's `Role` fixes its **team, color, AI, and troop tier** in one place,
//! so allegiance + difficulty read at a glance and stay consistent across episodes.
//! See docs/campaign-tutorial.md §11.

pub mod boudica;
pub mod dialog;

use sow_core::game_config::ScriptedSpawn;
use sow_core::player::{Civilization, Leader};
use sow_core::protocol::Team;

/// A faction's role fixes its team, color, AI, and troop tier. The difficulty ladder runs
/// Independent (500) → Vassal (1 000) → Boss (2 500) → BigBoss (5 000); the player (Boudica)
/// starts at 1 000 and grows by conquest, so the ladder climbs as the campaign progresses.
#[derive(Clone, Copy, PartialEq)]
pub enum Role {
    /// Boudica's kin/allies — Team Red, Boudica's color, passive (config troops).
    Kin,
    /// Lone clan — gray, no team, passive, **500** (the easy first-blood targets).
    Independent,
    /// Rome's client tribe — Team Blue, passive, **1 000** (a "vassal" of the bosses).
    Vassal,
    /// A Roman city — Team Blue, expanding nation, **2 500**.
    Boss,
    /// Rome itself — Team Blue, expanding nation, **5 000** (the apex).
    BigBoss,
    /// Unaligned bystander — its own color, no team, passive, **500**.
    Neutral,
}

impl Role {
    /// **Starting** troop count for this tier — the unit then grows normally with territory
    /// (so an expanding tribe's nameplate climbs, not freezes). The ladder is the head start.
    fn troops(self) -> Option<f64> {
        match self {
            Role::Kin | Role::Independent | Role::Neutral => Some(500.0),
            Role::Vassal => Some(1000.0),
            Role::Boss => Some(2500.0),
            Role::BigBoss => Some(5000.0),
        }
    }
    /// **Hard** max-troop ceiling. Currently unused (`None` for all) — every faction grows
    /// naturally with territory, so nameplates track reality (no frozen-at-500 look). Kept as a
    /// knob in case a future flavor unit must stay pinned; allies stay small by being passive +
    /// starting at 500, not by a hard cap.
    fn troop_cap(self) -> Option<f64> {
        match self {
            _ => None,
        }
    }
    /// `(team, expanding-nation?)`. Only the bosses + big boss actively expand.
    fn team_and_ai(self) -> (Option<Team>, bool) {
        match self {
            Role::Kin => (Some(Team::Red), false),
            Role::Vassal => (Some(Team::Blue), false),
            Role::Boss | Role::BigBoss => (Some(Team::Blue), true),
            Role::Independent | Role::Neutral => (None, false),
        }
    }
}

/// One placed faction in an episode roster.
pub struct Faction {
    pub name: &'static str,
    pub x: u32,
    pub y: u32,
    pub role: Role,
    pub civ: Civilization,
}

// Roster builders — keep episode files a readable table; civ is implied by role.
/// Boudica's kin/ally (Red, passive).
pub fn kin(name: &'static str, x: u32, y: u32) -> Faction {
    Faction { name, x, y, role: Role::Kin, civ: Civilization::Iceni }
}
/// A lone clan — gray, neutral, weak (500): the first-blood targets.
pub fn independent(name: &'static str, x: u32, y: u32) -> Faction {
    Faction { name, x, y, role: Role::Independent, civ: Civilization::Gallic }
}
/// Rome's client tribe — Blue, passive (1 000).
pub fn vassal(name: &'static str, x: u32, y: u32) -> Faction {
    Faction { name, x, y, role: Role::Vassal, civ: Civilization::Gallic }
}
/// A Roman city boss — Blue, expanding nation (2 500).
pub fn boss(name: &'static str, x: u32, y: u32) -> Faction {
    Faction { name, x, y, role: Role::Boss, civ: Civilization::Rome }
}
/// Rome itself — Blue, expanding nation (5 000): the big boss.
pub fn big_boss(name: &'static str, x: u32, y: u32) -> Faction {
    Faction { name, x, y, role: Role::BigBoss, civ: Civilization::Rome }
}
/// An unaligned bystander (own color, no team).
pub fn neutral(name: &'static str, x: u32, y: u32) -> Faction {
    Faction { name, x, y, role: Role::Neutral, civ: Civilization::Gallic }
}

/// Boudica's territory color — kin share it so the rebellion reads as one bloc.
/// (Mirrors `Leader::Boudica.filler_rgb()`.)
pub const ALLY_COLOR: [f32; 3] = [0.88, 0.42, 0.12];
const ROME_BLUE: [f32; 3] = [0.20, 0.45, 0.95];
const TRIBE_GRAY: [f32; 3] = [0.58, 0.58, 0.62];

/// Distinct own-colors for the neutral bystander factions (Welsh tribes, Gaul).
const NEUTRAL_PALETTE: [[f32; 3]; 6] = [
    [0.30, 0.65, 0.45], // green
    [0.66, 0.55, 0.25], // ochre
    [0.55, 0.35, 0.66], // purple
    [0.25, 0.60, 0.62], // teal
    [0.72, 0.45, 0.40], // clay
    [0.45, 0.50, 0.72], // slate
];

/// The human's team in the rebellion (Boudica leads Red).
pub const PLAYER_TEAM: Team = Team::Red;

/// Log the episode roster grouped by allegiance, so the console shows at a glance who is on
/// which side before the engine places them. (The engine then logs each actual placement.)
pub fn log_plan(episode: &str, player_spawn: (u32, u32), factions: &[Faction]) {
    let join = |roles: &[Role]| -> String {
        let v: Vec<&str> = factions
            .iter()
            .filter(|f| roles.contains(&f.role))
            .map(|f| f.name)
            .collect();
        if v.is_empty() {
            "(none)".into()
        } else {
            v.join(", ")
        }
    };
    log::info!(
        "campaign: {} — player (Boudica/Iceni, 1000) spawns at Norfolk ({},{}); {} scripted bots",
        episode,
        player_spawn.0,
        player_spawn.1,
        factions.len()
    );
    log::info!("campaign:   TEAM RED (us): [player] Boudica + kin {}", join(&[Role::Kin]));
    log::info!("campaign:   TEAM BLUE (Rome): big-boss {} | bosses {} | vassals {}",
        join(&[Role::BigBoss]), join(&[Role::Boss]), join(&[Role::Vassal]));
    log::info!("campaign:   INDEPENDENT (gray, 500): {}", join(&[Role::Independent]));
    log::info!("campaign:   NEUTRAL (own colors): {}", join(&[Role::Neutral]));
}

/// Turn an episode's faction list into engine-ready scripted spawns (team + color + tier).
pub fn to_scripted(factions: &[Faction]) -> Vec<ScriptedSpawn> {
    let mut neutral_i = 0usize;
    factions
        .iter()
        .map(|f| {
            let (team, is_nation) = f.role.team_and_ai();
            let color = match f.role {
                Role::Kin => ALLY_COLOR,
                Role::Vassal | Role::Boss | Role::BigBoss => ROME_BLUE,
                Role::Independent => TRIBE_GRAY,
                Role::Neutral => {
                    let c = NEUTRAL_PALETTE[neutral_i % NEUTRAL_PALETTE.len()];
                    neutral_i += 1;
                    c
                }
            };
            let leader = match f.role {
                Role::Boss | Role::BigBoss => Leader::Caesar,
                Role::Kin => Leader::Boudica,
                _ => Leader::default(),
            };
            ScriptedSpawn {
                name: f.name.to_string(),
                x: f.x,
                y: f.y,
                color,
                team,
                leader,
                civilization: f.civ,
                is_nation,
                troops: f.role.troops(),
                troop_cap: f.role.troop_cap(),
            }
        })
        .collect()
}
