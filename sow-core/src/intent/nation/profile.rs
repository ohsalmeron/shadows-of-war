use crate::game_config::BotDifficulty;
use crate::player::PlayerType;
use crate::protocol::GameplayIntent;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum BotDecisionKind {
    Build = 0,
    Attack = 1,
}

#[derive(Clone, Debug)]
pub(super) struct BotDecision {
    pub(super) bot_id: u16,
    pub(super) kind: BotDecisionKind,
    pub(super) intent: GameplayIntent,
}

// ──────────────────────────────────────────────────────────────────────────
// AI TIER — the single source of truth for "what is this entity".
//
// Before this refactor, personality was keyed on `bot_id % N` arithmetic
// left over from when `Bot` was the only AI type. That produced accidental
// élites (e.g. tribe id 400 → IQ 130-181, élite profile, no income handicap)
// and accidental fools ( ghosts with small ids → bottom-tier profile), which
// inverted the entire food chain (tribes eating nations/ghosts).
//
// Tier is now derived from `(player_type, is_ai_controlled)` — the real
// discriminators. bot_id survives only as RNG seed for *intra-tier*
// variety (cadence jitter, profile sub-pick), never as a tier switch.
//
// Food chain (per design):
//   Ghost/Human = TOP    (the goats; pro teammate / the player)
//   Nation      = NEXT   (mid-game food)
//   Tribe       = LOWEST (early-game food; passive vs players on Vanilla)
// ──────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AiTier {
    /// `Human` + `is_ai_controlled` — top-IQ pro teammate ("ghost").
    Ghost,
    /// `Nation` — smart, mid-game challengers.
    Nation,
    /// `Bot` aka tribes — lowest IQ, passive food on default difficulty.
    Tribe,
}

/// Resolve the AI tier for an entity. Returns `None` for real humans (no AI brain).
///
/// This is the ONE function that says what kind of AI something is. Every
// other site (cadence, combat profile, income handicap, send-cost, structure
// targets) must read from this — never re-derive from `bot_id` again.
pub fn ai_tier(player_type: PlayerType, is_ai_controlled: bool) -> Option<AiTier> {
    match (player_type, is_ai_controlled) {
        (PlayerType::Human, false) => None, // real human, no AI
        (PlayerType::Human, true) => Some(AiTier::Ghost),
        (PlayerType::Nation, _) => Some(AiTier::Nation),
        (PlayerType::Bot, _) => Some(AiTier::Tribe),
    }
}

/// Combat/expand posture for an AI entity. One lookup by tier — no bot_id arithmetic.
#[derive(Clone, Copy, Debug)]
pub(super) struct BotAiProfile {
    pub(super) trigger_ratio: f64,
    pub(super) reserve_ratio: f64,
    pub(super) expand_ratio: f64,
    pub(super) refuse_human_chance: i32,
    /// Tribe-only: on `Vanilla` (default MP) tribes don't initiate attacks
    /// against players; they still retaliate. On `Terminator` they hunt.
    pub(super) attacks_players: bool,
}

impl BotAiProfile {
    /// Ghost = top élite: attacks at 5% of cap, no reserve fear, never refuses.
    const GHOST: BotAiProfile = BotAiProfile {
        trigger_ratio: 0.05,
        reserve_ratio: 0.02,
        expand_ratio: 0.02,
        refuse_human_chance: 0,
        attacks_players: true,
    };

    /// Nation = smart but below ghost: 3 internal sub-tiers by `bot_id % 3`
    /// for organic variety (all still "smart"); trigger 0.30-0.55.
    fn nation(bot_id: u16) -> BotAiProfile {
        match bot_id % 3 {
            0 => BotAiProfile {
                trigger_ratio: 0.30,
                reserve_ratio: 0.10,
                expand_ratio: 0.10,
                refuse_human_chance: 10,
                attacks_players: true,
            },
            1 => BotAiProfile {
                trigger_ratio: 0.45,
                reserve_ratio: 0.20,
                expand_ratio: 0.15,
                refuse_human_chance: 20,
                attacks_players: true,
            },
            _ => BotAiProfile {
                trigger_ratio: 0.55,
                reserve_ratio: 0.30,
                expand_ratio: 0.20,
                refuse_human_chance: 30,
                attacks_players: true,
            },
        }
    }

    /// Tribe = passive food on `Vanilla`, hunters on `Terminator`.
    fn tribe(difficulty: BotDifficulty) -> BotAiProfile {
        match difficulty {
            BotDifficulty::Vanilla => BotAiProfile {
                trigger_ratio: 0.75, // ignored while attacks_players=false
                reserve_ratio: 0.50,
                expand_ratio: 0.10,
                refuse_human_chance: 100,
                attacks_players: false,
            },
            BotDifficulty::Terminator => BotAiProfile {
                trigger_ratio: 0.50,
                reserve_ratio: 0.35,
                expand_ratio: 0.15,
                refuse_human_chance: 50,
                attacks_players: true,
            },
        }
    }
}

/// Lookup table by tier. The old `get_bot_ai_profile(bot_id, is_nation)`
/// is replaced by this single dispatch.
pub(super) fn ai_profile_for(
    tier: AiTier,
    bot_id: u16,
    difficulty: BotDifficulty,
) -> BotAiProfile {
    match tier {
        AiTier::Ghost => BotAiProfile::GHOST,
        AiTier::Nation => BotAiProfile::nation(bot_id),
        AiTier::Tribe => BotAiProfile::tribe(difficulty),
    }
}

/// Describes which behaviours an AI entity should run this tick.
pub(super) struct AiSlot {
    pub(super) bot_id: u16,
    pub(super) tier: AiTier,
    pub(super) do_attack: bool,
    pub(super) do_structures: bool,
    pub(super) profile: BotAiProfile,
}
