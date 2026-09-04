//! Lifetime player stats persisted via the CrazyGames SDK Data module and sow-database.

use sow_core::player::Leader;
use serde::Deserialize;

pub const STORAGE_KEY: &str = "sow_player_progress";

#[derive(Default, Clone, Copy, Debug)]
pub struct SessionDefeats {
    pub players: u32,
    pub empires: u32,
    pub tribes: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone, Debug, PartialEq)]
pub struct PlayerProgress {
    pub xp: u32,
    pub level: u32,
    pub wins: u32,
    pub matches_played: u32,
    #[serde(alias = "players_killed")]
    pub players_defeated: u32,
    #[serde(alias = "empires_killed")]
    pub empires_defeated: u32,
    #[serde(alias = "tribes_killed")]
    pub tribes_defeated: u32,
    #[serde(default, deserialize_with = "deserialize_leader")]
    pub preferred_leader: Option<Leader>,
    pub intro_completed: Option<bool>,
    #[serde(default)]
    pub kills: u32,
    #[serde(default)]
    pub deaths: u32,
    #[serde(default)]
    pub assists: u32,
    #[serde(default)]
    pub leader_xp: std::collections::BTreeMap<String, u32>,
    #[serde(default)]
    pub laurels: u64,
    #[serde(default)]
    pub gems: u64,
    #[serde(default)]
    pub owned_leaders: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub owned_skins: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub selected_skin: Option<String>,
}

fn deserialize_leader<'de, D>(deserializer: D) -> Result<Option<Leader>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .as_deref()
        .map(|value| {
            sow_data::commerce::leader_from_id(value)
                .ok_or_else(|| serde::de::Error::custom("unknown leader"))
        })
        .transpose()
}

#[derive(Clone, Debug)]
pub enum DbEvent {
    ProfileLoaded {
        progress: PlayerProgress,
        account_id: String,
        public_id: Option<String>,
        display_name: String,
        provider: String,
        request_id: u64,
    },
    DisplayNameSaved {
        account_id: String,
        display_name: String,
        request_id: u64,
    },
    DisplayNameSaveFailed {
        request_id: u64,
        status: Option<u16>,
    },
    LoadFailed {
        request_id: u64,
        status: Option<u16>,
    },
    TutorialCompletionFailed {
        request_id: u64,
        status: Option<u16>,
    },
    NativeProfileLoaded {
        public_id: String,
        view: sow_data::profile::PublicProfileView,
    },
    NativeProfileLoadFailed {
        public_id: String,
        status: Option<u16>,
    },
    NativeProfileHistoryLoaded {
        public_id: String,
        items: Vec<sow_data::profile::PublicMatchSummary>,
        next_cursor: Option<usize>,
    },
    NativeProfileRatingsLoaded {
        public_id: String,
        items: Vec<sow_data::profile::PublicRatingView>,
    },
    NativeProfileSearchLoaded {
        query: String,
        items: Vec<sow_data::profile::PublicProfileSummary>,
    },
    NativeMatchDetailLoaded {
        match_id: String,
        detail: sow_data::profile::PublicMatchDetail,
    },
    NativeProfileOperationFailed {
        public_id: Option<String>,
        operation: String,
        message: String,
    },
    StoreProfileLoaded {
        account_id: String,
        progress: PlayerProgress,
        operation: String,
    },
    StoreActionFailed {
        operation: String,
        status: Option<u16>,
        message: String,
    },
}

impl PlayerProgress {
    pub fn is_first_game(&self) -> bool {
        self.matches_played == 0 && !self.intro_completed.unwrap_or(false)
    }

    pub fn complete_intro(&mut self) {
        self.intro_completed = Some(true);
    }

    pub fn apply_reward(
        &mut self,
        leader: Leader,
        reward: sow_data::rewards::MatchReward,
    ) {
        self.add_xp(reward.xp);
        let entry = self
            .leader_xp
            .entry(leader.name().to_string())
            .or_default();
        *entry = entry.saturating_add(reward.leader_xp);
        self.laurels = self.laurels.saturating_add(reward.laurels);
    }

    pub fn complete_tutorial_with_reward(&mut self) -> bool {
        if self.intro_completed.unwrap_or(false) {
            return false;
        }
        self.complete_intro();
        self.apply_reward(
            Leader::Boudica,
            sow_data::rewards::calculate(sow_data::rewards::RewardInput {
                tutorial: true,
                ..Default::default()
            }),
        );
        true
    }

    pub fn has_history(&self) -> bool {
        self.matches_played > 0
            || self.wins > 0
            || self.xp > 0
            || self.preferred_leader.is_some()
            || self.intro_completed.unwrap_or(false)
            || self.laurels > 0
            || self.gems > 0
            || !self.owned_leaders.is_empty()
            || !self.owned_skins.is_empty()
            || !self.leader_xp.is_empty()
    }

    /// Prefer cloud profile when it has history; otherwise keep local/CG portal data.
    pub fn merge_boot_profile(&mut self, cloud: PlayerProgress) {
        if cloud.has_history() || !self.has_history() {
            *self = cloud;
        }
    }

    pub fn sync_level(&mut self) {
        self.level = 1 + self.xp / 100;
    }

    pub fn add_xp(&mut self, amount: u32) {
        self.xp = self.xp.saturating_add(amount);
        self.sync_level();
    }

    pub fn record_match_with_kda(
        &mut self,
        won: bool,
        defeats: SessionDefeats,
        kills: u32,
        deaths: u32,
        assists: u32,
    ) {
        self.matches_played = self.matches_played.saturating_add(1);
        if won {
            self.wins = self.wins.saturating_add(1);
        }
        self.players_defeated = self.players_defeated.saturating_add(defeats.players);
        self.empires_defeated = self.empires_defeated.saturating_add(defeats.empires);
        self.tribes_defeated = self.tribes_defeated.saturating_add(defeats.tribes);
        self.kills = self.kills.saturating_add(kills);
        self.deaths = self.deaths.saturating_add(deaths);
        self.assists = self.assists.saturating_add(assists);

        let reward = sow_data::rewards::calculate(sow_data::rewards::RewardInput {
            won,
            players_defeated: defeats.players,
            empires_defeated: defeats.empires,
            tribes_defeated: defeats.tribes,
            kills,
            assists,
            ..Default::default()
        });
        self.apply_reward(
            self.preferred_leader.unwrap_or(Leader::Caesar),
            reward,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{PlayerProgress, SessionDefeats};
    use sow_core::player::Leader;

    #[test]
    fn tutorial_completion_is_idempotent_and_persists_reward() {
        let mut progress = PlayerProgress::default();
        assert!(progress.complete_tutorial_with_reward());
        assert!(!progress.complete_tutorial_with_reward());
        assert_eq!(progress.intro_completed, Some(true));
        assert_eq!(progress.xp, 100);
        assert_eq!(progress.laurels, 100);
        assert_eq!(progress.leader_xp.get("Boudica"), Some(&100));
    }

    #[test]
    fn match_reward_tracks_the_selected_leader_without_changing_gold() {
        let mut progress = PlayerProgress {
            preferred_leader: Some(Leader::Boudica),
            ..Default::default()
        };
        progress.record_match_with_kda(
            true,
            SessionDefeats {
                players: 1,
                ..Default::default()
            },
            2,
            1,
            1,
        );
        assert_eq!(progress.matches_played, 1);
        assert_eq!(progress.leader_xp.get("Boudica"), Some(&140));
        assert_eq!(progress.laurels, 106);
    }
}
