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
    /// One terse line under the objective row that says what to *do* and what the bar counts.
    /// Keep it to a single line at the panel's width (~30 chars).
    pub hint: &'static str,
    pub advance: Trigger,
}

// ponytail: inline EN strings = fastest script-iteration loop; move to sow-i18n once the
// script settles. Each entry is one objective; the tutorial NEVER ends — just append more
// objectives here as we build them out.
// Guide: 9 trials — start with expansion (tap to claim), learn combat (drag to attack),
// unite the east, then burn Rome's cities and ambush a legion. Final modal releases to
// the main menu with 100 Laurels; Stay keeps you fighting for honor.
pub(super) const CHAPTER_1: &[Step] = &[
    Step {
        title: "Rise of the Iceni",
        body: "Welcome to the campaign. Start by tapping unclaimed land beyond your border to capture territory. Each tile generates troops and gold.",
        hint: "Tap unclaimed land to expand territory",
        advance: Trigger::TilesGained(256),
    },
    Step {
        title: "Grow the Warband",
        body: "Territory scales your economy. Continue expanding outward to increase troop generation and gold income.",
        hint: "Expand to reach 1,024 tiles held",
        advance: Trigger::TilesGained(1024),
    },
    Step {
        title: "First Contact",
        body: "Enemy outposts control adjacent territory. Learn combat: select your forces and drag into enemy borders to attack.",
        hint: "Drag to attack an outpost — defeat 1 enemy",
        advance: Trigger::TribesEaten(1),
    },
    Step {
        title: "Unite the East",
        body: "Neutralize surrounding regional outposts to secure the eastern front under your banner.",
        hint: "Defeat 4 regional outposts",
        advance: Trigger::TribesEaten(4),
    },
    Step {
        title: "Siege of Camulodunum",
        body: "Siege mechanics: surround Camulodunum and cut off reinforcements to capture the settlement.",
        hint: "Capture Camulodunum to the south",
        advance: Trigger::DefeatedPlayer("Camulodunum"),
    },
    Step {
        title: "Advance on Londinium",
        body: "Londinium controls central trade. Capture the settlement to secure the river crossing.",
        hint: "Capture Londinium",
        advance: Trigger::DefeatedPlayer("Londinium"),
    },
    Step {
        title: "Capture Verulamium",
        body: "Verulamium on Watling Street. Capture the settlement to cut imperial supply lines.",
        hint: "Capture Verulamium",
        advance: Trigger::DefeatedPlayer("Verulamium"),
    },
    Step {
        title: "Ambush the Ninth",
        body: "Legio IX Hispana is advancing from the north. Intercept and defeat their forces.",
        hint: "Intercept Legio IX Hispana",
        advance: Trigger::DefeatedPlayer("Legio IX Hispana"),
    },
    // Terminal step: reaching it pops the Final Battle modal (Continue / Stay and fight) in `mod.rs`.
    // Its trigger never gates progression (it's last), but targeting Paulinus keeps the objective row
    // honest if the player stays and actually beats him. Continue releases to the main menu with
    // 100 Laurels earned; Stay keeps you on the map for honor.
    Step {
        title: "The Final Battle",
        body: "Tutorial complete. You have mastered expansion, combat, and siege. Suetonius Paulinus approaches with the main imperial force. Continue to return to the main menu (100 Laurels earned) or Stay to finish the engagement.",
        hint: "Continue to menu or defeat Paulinus",
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
