//! Episode 1 — Boudica's Rebellion on the `boudica` map (896×504; x→east, y→south).
//! Coordinates are real map anchors where they exist (Norfolk, the Roman cities, Devon, Powys,
//! Pas-de-Calais) and geographic estimates otherwise; the engine snaps each to the nearest free
//! land tile. See docs/campaign-tutorial.md §11.
//!
//! Difficulty ladder (troops): Boudica **1 000** → independent clans **500** → Roman vassals
//! **1 000** → city bosses **2 500** → Rome (big boss) **5 000**. The player grows by conquest,
//! so the ladder climbs as you go. Allegiance:
//! - **Red (us)**: Boudica + Iceni kin (Trinovantes + homeland sites), spread out.
//! - **Blue (Rome)**: Rome (big boss) + its city bosses + client-tribe vassals — one bloc.
//! - **Independent (gray)**: Caesar's 54 BC kneelers (Cassi/Bibroci/Ancalites/Segontiaci),
//!   ringing Norfolk **tight** as the first-blood targets.
//! - **Neutral (own colors)**: the unaligned British + Welsh tribes + Gaul.

use super::{big_boss, boss, independent, kin, neutral, vassal, Faction};
use sow_core::game_config::ScriptedSpawn;

/// Boudica (the player) spawns at the Iceni homeland: Norfolk.
pub const PLAYER_SPAWN: (u32, u32) = (696, 45);

/// The episode roster — who stands where, on whose side, and at what strength (via `Role`).
pub fn factions() -> Vec<Faction> {
    vec![
        // — First blood: Caesar's 54 BC kneelers (gray, start 500 then grow). Staggered
        //   distances (d≈20/43/58/84) + spread directions so you meet them one at a time —
        independent("Cassi", 693, 65),       // d≈20, due S — immediate first contact
        independent("Bibroci", 668, 78),     // d≈43, SW
        independent("Ancalites", 715, 100),  // d≈58, SE
        independent("Segontiaci", 673, 126), // d≈84, far SW
        // — Kin: Boudica's side (Red), spread out; Trinovantes far south in Essex —
        kin("Venta Icenorum", 640, 110), // the Iceni capital
        kin("Snettisham", 615, 50),      // Iceni gold-hoard sanctuary (the Wash)
        kin("Thetford", 590, 130),       // Iceni stronghold
        kin("Stonea", 540, 110),         // Fenland fort (the earlier Iceni revolt)
        kin("Trinovantes", 600, 200),    // sworn ally, far south (Essex, by Camulodunum)
        // — Rome's vassals: pro-Roman client tribes (Blue, 1 000) —
        vassal("Catuvellauni", 490, 235), // Romanised, around Verulamium
        vassal("Atrebates", 444, 370),    // Cogidubnus' client kingdom
        vassal("Cantiaci", 655, 345),     // Kent, gateway to Rome
        // — City bosses (Blue nation, 2 500): the three Boudica historically sacked —
        boss("Camulodunum", 646, 230),
        boss("Londinium", 576, 300),
        boss("Verulamium", 536, 270),
        // — Big boss (Blue nation, 5 000): Rome, deep in the province —
        big_boss("Rome", 470, 300),
        // — Neutral bystanders (own colors, 500): unaligned British + Welsh + Gaul —
        neutral("Corieltauvi", 515, 115),
        neutral("Cornovii", 330, 155),
        neutral("Dobunni", 360, 255),
        neutral("Durotriges", 345, 400),
        neutral("Dumnonii", 163, 433),
        neutral("Silures", 235, 245),
        neutral("Demetae", 110, 205),
        neutral("Ordovices", 198, 119),
        neutral("Deceangli", 255, 85),
        neutral("Morini", 835, 465),
    ]
}

/// Engine-ready roster for `GameConfig.scripted_spawns`.
pub fn scripted_spawns() -> Vec<ScriptedSpawn> {
    super::to_scripted(&factions())
}

#[cfg(test)]
mod tests {
    use super::super::Role;
    use super::*;
    use sow_core::protocol::Team;

    #[test]
    fn roster_is_staged_for_the_tutorial() {
        let f = factions();
        let by_role = |r: Role| f.iter().filter(|x| x.role == r).count();
        assert_eq!(by_role(Role::Kin), 5, "Trinovantes + 4 Iceni sites");
        assert_eq!(by_role(Role::Independent), 4, "the four first-blood kneelers");
        assert_eq!(by_role(Role::Boss), 3, "the three sacked cities");
        assert_eq!(by_role(Role::BigBoss), 1, "Rome");

        let s = scripted_spawns();
        let spawn = |n: &str| s.iter().find(|x| x.name == n).unwrap();
        // Troop ladder is the STARTING head start; only kin are hard-capped (stay weak),
        // enemies grow normally (no cap) so expanding tribes' nameplates climb.
        assert_eq!(spawn("Cassi").troops, Some(500.0));
        // Nobody is hard-capped now — all grow with territory (no frozen nameplate).
        assert_eq!(spawn("Cassi").troop_cap, None, "enemy clans grow");
        assert_eq!(spawn("Trinovantes").troop_cap, None, "kin grow too (passive, start 500)");
        assert_eq!(spawn("Catuvellauni").troops, Some(1000.0));
        assert_eq!(spawn("Camulodunum").troops, Some(2500.0));
        assert_eq!(spawn("Rome").troops, Some(5000.0));
        // Rome's bloc is all Blue; kin are passive Red; bosses expand.
        assert_eq!(spawn("Rome").team, Some(Team::Blue));
        assert_eq!(spawn("Catuvellauni").team, Some(Team::Blue));
        assert!(!spawn("Catuvellauni").is_nation, "vassals are passive");
        assert!(spawn("Rome").is_nation, "the big boss expands");
        let tr = spawn("Trinovantes");
        assert_eq!(tr.team, Some(Team::Red));
        assert!(!tr.is_nation, "kin are passive");
        assert_eq!(spawn("Cassi").team, None, "clans are independent");
    }
}
