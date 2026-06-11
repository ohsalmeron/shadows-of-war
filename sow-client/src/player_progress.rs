//! Lifetime player stats persisted via the CrazyGames SDK Data module and sow-database.

use sow_core::player::Leader;
use sha2::{Sha256, Digest};

pub const STORAGE_KEY: &str = "sow_player_progress";

const XP_WIN: u32 = 100;
const XP_MATCH: u32 = 20;

const XP_PER_PLAYER: u32 = 15;
const XP_PER_EMPIRE: u32 = 8;
const XP_PER_TRIBE: u32 = 2;

/// Build-time salt configured in .env for production builds.
/// Defaults to a dev salt for frictionless local open-source development.
pub const CLIENT_SALT: &str = match option_env!("SOW_CLIENT_SALT") {
    Some(salt) => salt,
    None => "sow_dev_salt_abc123",
};

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
}

#[derive(Clone, Debug)]
pub struct LinkConflictInfo {
    pub current_account_id: String,
    pub existing_account_id: String,
    pub current_level: u32,
    pub existing_level: u32,
    pub target_provider: String,
    pub target_external_id: String,
}

#[derive(Clone, Debug)]
pub enum DbEvent {
    ProfileLoaded {
        progress: PlayerProgress,
        account_id: String,
        provider: String,
    },
    LoadFailed,
    LinkConflict(LinkConflictInfo),
    LinkResolved {
        progress: PlayerProgress,
        account_id: String,
        provider: String,
    },
}

/// Computes SHA-256 signature of data with compile-time salt
pub fn compute_client_signature(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hasher.update(CLIENT_SALT.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

impl PlayerProgress {
    pub fn has_history(&self) -> bool {
        self.matches_played > 0
            || self.wins > 0
            || self.xp > 0
            || self.preferred_leader.is_some()
    }

    /// Prefer cloud profile when it has history; otherwise keep local/CG portal data.
    pub fn merge_boot_profile(&mut self, cloud: PlayerProgress) {
        if cloud.has_history() {
            *self = cloud;
        } else if !self.has_history() {
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

    pub fn record_match(&mut self, won: bool, defeats: SessionDefeats) {
        self.matches_played = self.matches_played.saturating_add(1);
        if won {
            self.wins = self.wins.saturating_add(1);
        }
        self.players_defeated = self.players_defeated.saturating_add(defeats.players);
        self.empires_defeated = self.empires_defeated.saturating_add(defeats.empires);
        self.tribes_defeated = self.tribes_defeated.saturating_add(defeats.tribes);

        let mut xp_gain = XP_MATCH;
        xp_gain = xp_gain.saturating_add(defeats.players.saturating_mul(XP_PER_PLAYER));
        xp_gain = xp_gain.saturating_add(defeats.empires.saturating_mul(XP_PER_EMPIRE));
        xp_gain = xp_gain.saturating_add(defeats.tribes.saturating_mul(XP_PER_TRIBE));

        if won {
            xp_gain = xp_gain.saturating_add(XP_WIN);
        }
        self.add_xp(xp_gain);
    }
}
