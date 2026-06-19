
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

/// Describes which behaviours an AI entity should run this tick.
pub(super) struct AiSlot {
    pub(super) bot_id: u16,
    pub(super) is_nation: bool,
    pub(super) do_attack: bool,
    pub(super) do_structures: bool,
    pub(super) profile: BotAiProfile,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct BotAiProfile {
    pub(super) trigger_ratio: f64,
    pub(super) reserve_ratio: f64,
    pub(super) expand_ratio: f64,
    pub(super) refuse_human_chance: i32,
}

pub(super) fn get_bot_ai_profile(bot_id: u16, is_nation: bool) -> BotAiProfile {
    let is_smart = is_nation || bot_id.is_multiple_of(100);
    if is_smart && bot_id.is_multiple_of(8) {
        BotAiProfile {
            trigger_ratio: 0.10,
            reserve_ratio: 0.02,
            expand_ratio: 0.02,
            refuse_human_chance: 0,
        }
    } else if is_smart {
        match bot_id % 4 {
            0 => BotAiProfile {
                trigger_ratio: 0.45,
                reserve_ratio: 0.15,
                expand_ratio: 0.15,
                refuse_human_chance: 20,
            },
            1 => BotAiProfile {
                trigger_ratio: 0.65,
                reserve_ratio: 0.40,
                expand_ratio: 0.20,
                refuse_human_chance: 60,
            },
            2 => BotAiProfile {
                trigger_ratio: 0.55,
                reserve_ratio: 0.30,
                expand_ratio: 0.15,
                refuse_human_chance: 40,
            },
            _ => BotAiProfile {
                trigger_ratio: 0.75,
                reserve_ratio: 0.50,
                expand_ratio: 0.25,
                refuse_human_chance: 80,
            },
        }
    } else if bot_id.is_multiple_of(20) {
        BotAiProfile {
            trigger_ratio: 0.45,
            reserve_ratio: 0.15,
            expand_ratio: 0.10,
            refuse_human_chance: 0,
        }
    } else {
        match bot_id % 3 {
            0 => BotAiProfile {
                trigger_ratio: 0.75,
                reserve_ratio: 0.50,
                expand_ratio: 0.20,
                refuse_human_chance: 90,
            },
            1 => BotAiProfile {
                trigger_ratio: 0.65,
                reserve_ratio: 0.35,
                expand_ratio: 0.15,
                refuse_human_chance: 75,
            },
            _ => BotAiProfile {
                trigger_ratio: 0.70,
                reserve_ratio: 0.30,
                expand_ratio: 0.10,
                refuse_human_chance: 85,
            },
        }
    }
}

