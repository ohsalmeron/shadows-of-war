//! Campaign step data + objective evaluation. This is the *content* layer: the ordered
//! table of objectives the tutorial walks through. Append objectives here — the runner in
//! [`super`] interprets them generically, so growing the campaign is data, not new code.

use sow_core::player::Leader;

/// Portrait + narrator for the campaign dialog.
pub(super) const ADVISOR: Leader = Leader::Boudica;

/// What completes a step's objective. (Add variants as new mechanics need them — each must
/// map to a field that already exists on a `PlayerSnapshot`; see `objective_progress`.)
#[derive(Clone, Copy)]
pub(super) enum Trigger {
    /// Claim N tiles since spawn (cumulative; the spawn blob exceeds small absolutes).
    TilesGained(u32),
    /// Eliminate N players ("eat N tribes") — uses the human's cumulative kill count.
    TribesEaten(u32),
    /// Defeat a SPECIFIC faction (eliminated = present in the snapshot but no longer alive). The
    /// name must match a faction in the roster (`assets/campaign/boudica.json`); a test enforces it.
    DefeatedPlayer(&'static str),
}

/// Current/target progress for a step's objective. `is_defeated(name)` reports whether a named
/// faction has been eliminated (built from the live snapshot by the runner).
pub(super) fn objective_progress(
    advance: Trigger,
    gained: u32,
    kills: u32,
    is_defeated: &dyn Fn(&str) -> bool,
) -> (u32, u32) {
    match advance {
        Trigger::TilesGained(t) => (gained.min(t), t),
        Trigger::TribesEaten(t) => (kills.min(t), t),
        Trigger::DefeatedPlayer(name) => (u32::from(is_defeated(name)), 1),
    }
}

/// Every step is one objective: the modal states it, the player taps to close it, then goes
/// and completes it; completion opens the next step's modal.
pub(super) struct Step {
    /// Dialog title + objective row label.
    pub title: &'static str,
    /// Narration shown in the dialog.
    pub body: &'static str,
    pub advance: Trigger,
}

// ponytail: inline EN strings = fastest script-iteration loop; move to sow-i18n once the
// script settles. Each entry is one objective; the tutorial NEVER ends — just append more
// objectives here as we build them out.
pub(super) const CHAPTER_1: &[Step] = &[
    Step {
        title: "Rise of the Iceni",
        body: "I am Boudica. Rome thinks us weak — tap the wild land beyond our border and seize it.",
        advance: Trigger::TilesGained(256),
    },
    Step {
        title: "Grow the Warband",
        body: "Keep pushing outward — every tile feeds troops and gold into the revolt.",
        advance: Trigger::TilesGained(1024),
    },
    Step {
        title: "First Blood",
        body: "The Romans built outposts and farms on our land. Drag your warriors into them and blood your spears.",
        advance: Trigger::TribesEaten(1),
    },
    Step {
        title: "Unite the East",
        body: "Veterans, outposts, and kneeling nobles. Unite the east under one banner: devour all four.",
        advance: Trigger::TribesEaten(4),
    },
    Step {
        title: "The First Fire",
        body: "Camulodunum stands to our south. Burn it.",
        advance: Trigger::DefeatedPlayer("Camulodunum"),
    },
    Step {
        title: "The Second Fire",
        body: "Londinium, the heart of their trade. Burn it.",
        advance: Trigger::DefeatedPlayer("Londinium"),
    },
    Step {
        title: "The Third Fire",
        body: "Verulamium along Watling Street. Leave nothing but ash.",
        advance: Trigger::DefeatedPlayer("Verulamium"),
    },
    Step {
        title: "Ambush the Ninth",
        body: "Legio IX Hispana marches from the north. Cut them down.",
        advance: Trigger::DefeatedPlayer("Legio IX Hispana"),
    },
    // Terminal step: reaching it pops the Final Battle modal (Continue / Stay and fight) in `mod.rs`.
    // Its trigger never gates progression (it's last), but targeting Paulinus keeps the objective row
    // honest if the player stays and actually beats him.
    Step {
        title: "The Final Battle",
        body: "Congratulations, tutorial complete! Suetonius Paulinus returns from Wales with the XIV and XX Legions. This is where the history books say we fall. Will you stay and fight?",
        advance: Trigger::DefeatedPlayer("Legio XIV Gemina"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Poka-yoke: every faction a `DefeatedPlayer` objective targets must exist in the roster
    /// (`assets/campaign/boudica.json`). Rename/remove a boss in the editor and this fails the build
    /// instead of letting the campaign get stuck on an objective that can never complete.
    #[test]
    fn objective_targets_exist_in_roster() {
        let (factions, _) = crate::campaign::boudica::roster();
        let names: std::collections::HashSet<&str> =
            factions.iter().map(|f| f.name.as_str()).collect();
        for step in CHAPTER_1 {
            if let Trigger::DefeatedPlayer(target) = step.advance {
                assert!(
                    names.contains(target),
                    "objective '{}' targets '{}', which is not in the roster",
                    step.title,
                    target
                );
            }
        }
    }
}