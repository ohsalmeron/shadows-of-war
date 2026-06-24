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
}

/// Current/target progress for a step's objective, given live tile-gain and kill counts.
pub(super) fn objective_progress(advance: Trigger, gained: u32, kills: u32) -> (u32, u32) {
    match advance {
        Trigger::TilesGained(t) => (gained.min(t), t),
        Trigger::TribesEaten(t) => (kills.min(t), t),
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
        title: "First Blood — the Cassi",
        body: "The Cassi knelt to Caesar a hundred years past and still lick Roman boots. They are weak. Drag your warriors into them and blood your spears.",
        advance: Trigger::TribesEaten(1),
    },
    Step {
        title: "Bring the Kneelers to Heel",
        body: "Bibroci, Ancalites, Segontiaci — every clan that bowed to Rome. Each is stronger than the last. Unite the east under one banner: devour all four.",
        advance: Trigger::TribesEaten(4),
    },
];
