//! Lifetime player stats persisted via the CrazyGames SDK Data module and sow-database.

use sow_core::player::Leader;

pub const STORAGE_KEY: &str = "sow_player_progress";

const XP_WIN: u32 = 100;
const XP_MATCH: u32 = 20;

const XP_PER_PLAYER: u32 = 15;
const XP_PER_EMPIRE: u32 = 8;
const XP_PER_TRIBE: u32 = 2;
const XP_PER_ASSIST: u32 = 5;

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
    pub preferred_leader: Option<Leader>,
    pub intro_completed: Option<bool>,
    #[serde(default)]
    pub kills: u32,
    #[serde(default)]
    pub deaths: u32,
    #[serde(default)]
    pub assists: u32,
}

#[derive(Clone, Debug)]
pub enum DbEvent {
    ProfileLoaded {
        progress: PlayerProgress,
        account_id: String,
        provider: String,
    },
    LoadFailed,
}

impl PlayerProgress {
    pub fn is_first_game(&self) -> bool {
        self.matches_played == 0 && !self.intro_completed.unwrap_or(false)
    }

    pub fn complete_intro(&mut self) {
        self.intro_completed = Some(true);
    }

    pub fn has_history(&self) -> bool {
        self.matches_played > 0 || self.wins > 0 || self.xp > 0 || self.preferred_leader.is_some()
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

        let mut xp_gain = XP_MATCH;
        xp_gain = xp_gain.saturating_add(defeats.players.saturating_mul(XP_PER_PLAYER));
        xp_gain = xp_gain.saturating_add(defeats.empires.saturating_mul(XP_PER_EMPIRE));
        xp_gain = xp_gain.saturating_add(defeats.tribes.saturating_mul(XP_PER_TRIBE));
        xp_gain = xp_gain.saturating_add(assists.saturating_mul(XP_PER_ASSIST));

        if won {
            xp_gain = xp_gain.saturating_add(XP_WIN);
        }
        self.add_xp(xp_gain);
    }
}
