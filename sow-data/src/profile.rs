//! Durable public-profile records.
//!
//! Account aggregates remain in Valkey/`PLAYERS_TABLE`.  This module stores
//! the append-only pieces that cannot be reconstructed from an aggregate:
//! match history, public identity indexes, seasons, and ladder ratings.

use serde::{Deserialize, Serialize};

pub const CURRENT_SEASON_ID: u32 = 1;
pub const CURRENT_SEASON_NAME: &str = "Season 1";

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct LeaderCareerStats {
    pub matches_played: u32,
    pub wins: u32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub xp: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct MatchParticipantRecord {
    /// Internal account ID. Never serialize this record directly to a public
    /// response; the public DTOs below deliberately omit it.
    pub account_id: String,
    pub public_id: String,
    pub display_name: String,
    pub is_bot: bool,
    pub leader: Option<String>,
    pub team: Option<String>,
    pub placement: u16,
    pub won: bool,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub players_defeated: u32,
    pub empires_defeated: u32,
    pub tribes_defeated: u32,
    pub xp: u32,
    pub leader_xp: u32,
    pub laurels: u64,
    pub rating_delta: Option<i16>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct MatchRecord {
    pub schema_version: u8,
    pub match_id: String,
    pub season_id: u32,
    pub queue: String,
    pub mode: String,
    pub map_name: String,
    pub started_at: u64,
    pub completed_at: u64,
    pub duration_seconds: u32,
    pub winner_account_id: Option<String>,
    pub winning_team: Option<String>,
    pub verified: bool,
    pub rating_eligible: bool,
    pub participants: Vec<MatchParticipantRecord>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SeasonRating {
    pub schema_version: u8,
    pub account_id: String,
    pub season_id: u32,
    pub queue: String,
    pub mode: String,
    pub games_played: u32,
    pub wins: u32,
    pub mu: f64,
    pub sigma: f64,
    pub score: u16,
    pub tier: String,
    pub division: Option<String>,
    pub peak_score: u16,
    pub placements_complete: bool,
    pub updated_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SeasonRecord {
    pub id: u32,
    pub name: String,
    pub status: String,
    pub starts_at: u64,
    pub ends_at: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct PublicProfileIndex {
    pub account_id: String,
    pub public_id: String,
    pub display_name: String,
    pub kind: String,
    pub updated_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicProfileSummary {
    pub public_id: String,
    pub handle: String,
    pub display_name: String,
    pub level: u32,
    pub matches_played: u32,
    pub wins: u32,
    pub win_rate: f32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicLeaderSummary {
    pub leader: String,
    pub matches_played: u32,
    pub wins: u32,
    pub win_rate: f32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub xp: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicMatchSummary {
    pub match_id: String,
    pub completed_at: u64,
    pub queue: String,
    pub mode: String,
    pub map_name: String,
    pub duration_seconds: u32,
    pub placement: u16,
    pub won: bool,
    pub leader: Option<String>,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub players_defeated: u32,
    pub verified: bool,
    pub rating_delta: Option<i16>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicMatchParticipant {
    pub public_id: String,
    pub handle: String,
    pub is_bot: bool,
    pub leader: Option<String>,
    pub team: Option<String>,
    pub placement: u16,
    pub won: bool,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub players_defeated: u32,
    pub verified: bool,
    pub rating_delta: Option<i16>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicMatchDetail {
    pub match_id: String,
    pub completed_at: u64,
    pub queue: String,
    pub mode: String,
    pub map_name: String,
    pub duration_seconds: u32,
    pub winner_public_id: Option<String>,
    pub winning_team: Option<String>,
    pub verified: bool,
    pub rating_eligible: bool,
    pub participants: Vec<PublicMatchParticipant>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicProfileView {
    pub public_id: String,
    pub handle: String,
    pub display_name: String,
    pub level: u32,
    pub matches_played: u32,
    pub wins: u32,
    pub win_rate: f32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub players_defeated: u32,
    pub empires_defeated: u32,
    pub tribes_defeated: u32,
    pub preferred_leader: Option<String>,
    pub leaders: Vec<PublicLeaderSummary>,
    pub recent_matches: Vec<PublicMatchSummary>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicRatingView {
    pub season_id: u32,
    pub season_name: String,
    pub queue: String,
    pub mode: String,
    pub games_played: u32,
    pub wins: u32,
    pub placements_complete: bool,
    pub score: u16,
    pub tier: String,
    pub division: Option<String>,
    pub peak_score: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PublicLeaderboardEntry {
    pub rank: u32,
    pub public_id: String,
    pub handle: String,
    pub queue: String,
    pub mode: String,
    pub score: u16,
    pub tier: String,
    pub division: Option<String>,
    pub games_played: u32,
    pub wins: u32,
}

pub fn public_profile_id(account_id: &str) -> String {
    let digest = blake3::hash(account_id.as_bytes()).to_hex().to_string();
    format!("p_{}", &digest[..24])
}

pub fn public_handle(display_name: &str, public_id: &str) -> String {
    let suffix = public_id
        .trim_start_matches("p_")
        .chars()
        .take(4)
        .collect::<String>()
        .to_uppercase();
    format!("{}#{suffix}", display_name.trim())
}

pub fn win_rate(wins: u32, matches: u32) -> f32 {
    if matches == 0 {
        0.0
    } else {
        (wins as f32 / matches as f32 * 100.0).round() / 100.0
    }
}

pub fn kda(kills: u32, deaths: u32, assists: u32) -> f32 {
    ((kills + assists) as f32 / deaths.max(1) as f32 * 100.0).round() / 100.0
}

pub fn ladder_score(mu: f64, sigma: f64) -> u16 {
    let ordinal = mu - 3.0 * sigma;
    (1500.0 + ordinal * 50.0).round().clamp(800.0, 4000.0) as u16
}

pub fn tier_for_score(score: u16) -> (&'static str, Option<String>) {
    let tier = match score {
        0..=1199 => "Bronze",
        1200..=1599 => "Silver",
        1600..=1999 => "Gold",
        2000..=2399 => "Platinum",
        2400..=2799 => "Diamond",
        2800..=3199 => "Master",
        _ => "Grandmaster",
    };
    let division = if (800..2800).contains(&score) {
        let within = ((score.saturating_sub(800)) % 400) / 100;
        Some(
            match within {
                0 => "IV",
                1 => "III",
                2 => "II",
                _ => "I",
            }
            .to_string(),
        )
    } else {
        None
    };
    (tier, division)
}
