//! Shared, deterministic rewards math used by the client preview and database.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RewardInput {
    pub won: bool,
    pub players_defeated: u32,
    pub empires_defeated: u32,
    pub tribes_defeated: u32,
    pub kills: u32,
    pub assists: u32,
    pub tutorial: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatchReward {
    pub xp: u32,
    pub leader_xp: u32,
    pub laurels: u64,
}

const XP_MATCH: u32 = 20;
const XP_WIN: u32 = 100;
const XP_PER_PLAYER: u32 = 15;
const XP_PER_EMPIRE: u32 = 8;
const XP_PER_TRIBE: u32 = 2;
const XP_PER_ASSIST: u32 = 5;

const LAURELS_PARTICIPATION: u64 = 25;
const LAURELS_WIN: u64 = 75;
const LAURELS_PER_KILL: u64 = 2;
const LAURELS_PER_EMPIRE: u64 = 5;
const LAURELS_PER_ASSIST: u64 = 2;
const LAURELS_TUTORIAL: u64 = 100;

pub fn calculate(input: RewardInput) -> MatchReward {
    if input.tutorial {
        return MatchReward {
            xp: 100,
            leader_xp: 100,
            laurels: LAURELS_TUTORIAL,
        };
    }

    let mut xp = XP_MATCH;
    xp = xp.saturating_add(input.players_defeated.saturating_mul(XP_PER_PLAYER));
    xp = xp.saturating_add(input.empires_defeated.saturating_mul(XP_PER_EMPIRE));
    xp = xp.saturating_add(input.tribes_defeated.saturating_mul(XP_PER_TRIBE));
    xp = xp.saturating_add(input.assists.saturating_mul(XP_PER_ASSIST));
    if input.won {
        xp = xp.saturating_add(XP_WIN);
    }

    let mut laurels = LAURELS_PARTICIPATION;
    if input.won {
        laurels = laurels.saturating_add(LAURELS_WIN);
    }
    laurels = laurels.saturating_add((input.kills as u64).saturating_mul(LAURELS_PER_KILL));
    laurels = laurels.saturating_add(
        (input.empires_defeated as u64).saturating_mul(LAURELS_PER_EMPIRE),
    );
    laurels = laurels.saturating_add((input.assists as u64).saturating_mul(LAURELS_PER_ASSIST));

    MatchReward {
        xp,
        leader_xp: xp,
        laurels,
    }
}

pub fn canonical_leader_name(value: &str) -> Option<String> {
    Leader::ALL
        .iter()
        .find(|leader| leader.name() == value)
        .map(|leader| leader.name().to_string())
}

use crate::Leader;

#[cfg(test)]
mod tests {
    use super::{RewardInput, calculate, canonical_leader_name};

    #[test]
    fn reward_math_is_deterministic_for_win_and_loss() {
        let loss = calculate(RewardInput {
            kills: 3,
            assists: 2,
            ..Default::default()
        });
        assert_eq!(loss.xp, 30);
        assert_eq!(loss.leader_xp, 30);
        assert_eq!(loss.laurels, 35);

        let win = calculate(RewardInput {
            won: true,
            players_defeated: 2,
            empires_defeated: 1,
            assists: 1,
            ..Default::default()
        });
        assert_eq!(win.xp, 163);
        assert_eq!(win.leader_xp, 163);
        assert_eq!(win.laurels, 107);
    }

    #[test]
    fn tutorial_reward_is_fixed_and_leader_names_are_whitelisted() {
        assert_eq!(
            calculate(RewardInput {
                tutorial: true,
                ..Default::default()
            }),
            super::MatchReward {
                xp: 100,
                leader_xp: 100,
                laurels: 100,
            }
        );
        assert_eq!(canonical_leader_name("Boudica").as_deref(), Some("Boudica"));
        assert!(canonical_leader_name("not-a-leader").is_none());
    }
}
