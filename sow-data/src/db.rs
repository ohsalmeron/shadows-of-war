use crate::metadata_db::{
    MATCHES_TABLE, PLAYER_MATCH_INDEX_TABLE, PLAYERS_TABLE, PUBLIC_PROFILES_TABLE,
    SEASON_RATINGS_TABLE, SEASONS_TABLE,
};
use crate::profile::{
    LeaderCareerStats, MatchRecord, PublicLeaderboardEntry, PublicLeaderSummary,
    PublicMatchDetail, PublicMatchParticipant, PublicMatchSummary, PublicProfileIndex,
    PublicProfileSummary, PublicProfileView,
    PublicRatingView, SeasonRating, SeasonRecord, public_handle, public_profile_id, win_rate,
};
use log::{error, info};
use redb::ReadableTable;
use redis::{AsyncCommands, Client};
use serde::{Deserialize, Serialize};

const ANALYTICS_UNIQUE: &str = "sow:analytics:unique_users";
const ANALYTICS_ACTIVE_PREFIX: &str = "sow:analytics:active:";
const ANALYTICS_DAU_TTL_SECS: i64 = 35 * 24 * 3600;
const ANALYTICS_EVENT_COUNT_PREFIX: &str = "sow:analytics:event:";
const ANALYTICS_EVENT_USERS_PREFIX: &str = "sow:analytics:event_users:";
const ANALYTICS_COHORT_PREFIX: &str = "sow:analytics:cohort:";
const ANALYTICS_ACTIVATED_PREFIX: &str = "sow:analytics:activated:";
const ANALYTICS_RETENTION_TTL_SECS: i64 = 100 * 24 * 3600;

/// SET index of all bot account_ids — populated by `seed_bot_pool`, used for
/// pool introspection and analytics. Account records carry a canonical
/// display_name field; the bot allocator may choose its presentation name.
const BOT_POOL_KEY: &str = "sow:bot:pool";

const ACCOUNT_ID_HEX_LEN: usize = 32;
pub const DISPLAY_NAME_MAX_CHARS: usize = 16;

/// Generate the initial presentation name only when the client did not send one.
/// The account ID remains the sole stable identity key.
fn generated_display_name() -> String {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        % 1000;
    format!("ANON{suffix:03}")
}

/// Normalize a player-facing name without making it an identity key.
/// The account ID remains the stable identity; this value is presentation data.
pub fn normalize_display_name(value: &str) -> Result<String, &'static str> {
    let normalized: String = value
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(DISPLAY_NAME_MAX_CHARS)
        .collect();
    if normalized.is_empty() {
        return Err("display_name cannot be empty");
    }
    Ok(normalized)
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct SessionDefeats {
    pub players: u32,
    pub empires: u32,
    pub tribes: u32,
}

#[derive(Clone, Debug)]
pub struct MatchOutcomeKda {
    pub defeats: SessionDefeats,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub leader: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerProfile {
    pub xp: u32,
    pub level: u32,
    pub wins: u32,
    pub matches_played: u32,
    pub players_defeated: u32,
    pub empires_defeated: u32,
    pub tribes_defeated: u32,
    pub preferred_leader: Option<String>,
    #[serde(default)]
    pub kills: u32,
    #[serde(default)]
    pub deaths: u32,
    #[serde(default)]
    pub assists: u32,
    #[serde(default)]
    pub leader_xp: std::collections::BTreeMap<String, u32>,
    #[serde(default)]
    pub leader_stats: std::collections::BTreeMap<String, LeaderCareerStats>,
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
    #[serde(default)]
    pub processed_revenuecat_events: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub intro_completed: bool,
}

impl Default for PlayerProfile {
    fn default() -> Self {
        Self {
            xp: 0,
            level: 1,
            wins: 0,
            matches_played: 0,
            players_defeated: 0,
            empires_defeated: 0,
            tribes_defeated: 0,
            preferred_leader: None,
            kills: 0,
            deaths: 0,
            assists: 0,
            leader_xp: std::collections::BTreeMap::new(),
            leader_stats: std::collections::BTreeMap::new(),
            laurels: 0,
            gems: 0,
            owned_leaders: std::collections::BTreeSet::new(),
            owned_skins: std::collections::BTreeSet::new(),
            selected_skin: None,
            processed_revenuecat_events: std::collections::BTreeSet::new(),
            intro_completed: false,
        }
    }
}

impl PlayerProfile {
    pub fn for_account(account_id: &str) -> Self {
        let mut profile = Self::default();
        let leader = crate::commerce::assigned_leader_for_account(
            account_id,
            crate::commerce::current_rotation_period(),
        );
        profile.preferred_leader = Some(crate::commerce::leader_wire_id(leader).to_string());
        profile
    }

    pub fn sync_level(&mut self) {
        self.level = 1 + self.xp / 100;
    }

    pub fn add_xp(&mut self, amount: u32) {
        self.xp = self.xp.saturating_add(amount);
        self.sync_level();
    }

    pub fn apply_reward(
        &mut self,
        leader: &str,
        reward: crate::rewards::MatchReward,
    ) {
        self.add_xp(reward.xp);
        let entry = self.leader_xp.entry(leader.to_string()).or_default();
        *entry = entry.saturating_add(reward.leader_xp);
        let stats = self.leader_stats.entry(leader.to_string()).or_default();
        stats.xp = stats.xp.saturating_add(reward.leader_xp);
        self.laurels = self.laurels.saturating_add(reward.laurels);
    }

    pub fn record_match_with_kda(
        &mut self,
        won: bool,
        defeats: SessionDefeats,
        kills: u32,
        deaths: u32,
        assists: u32,
    ) {
        self.record_match_with_leader(won, defeats, kills, deaths, assists, None);
    }

    pub fn record_match_with_leader(
        &mut self,
        won: bool,
        defeats: SessionDefeats,
        kills: u32,
        deaths: u32,
        assists: u32,
        leader: Option<&str>,
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

        let reward = crate::rewards::calculate(crate::rewards::RewardInput {
            won,
            players_defeated: defeats.players,
            empires_defeated: defeats.empires,
            tribes_defeated: defeats.tribes,
            kills,
            assists,
            ..Default::default()
        });
        let leader = leader
            .and_then(crate::rewards::canonical_leader_name)
            .or_else(|| {
                self.preferred_leader
                    .as_deref()
                    .and_then(crate::rewards::canonical_leader_name)
            })
            .unwrap_or_else(|| "Caesar".to_string());
        let leader_stats = self.leader_stats.entry(leader.clone()).or_default();
        leader_stats.matches_played = leader_stats.matches_played.saturating_add(1);
        if won {
            leader_stats.wins = leader_stats.wins.saturating_add(1);
        }
        leader_stats.kills = leader_stats.kills.saturating_add(kills);
        leader_stats.deaths = leader_stats.deaths.saturating_add(deaths);
        leader_stats.assists = leader_stats.assists.saturating_add(assists);
        self.apply_reward(&leader, reward);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerAccount {
    pub id: String, // Stable canonical internal account ID
    /// Stable public profile identifier safe to expose in URLs.
    #[serde(default)]
    pub public_id: String,
    /// Mutable player-facing name. Never used as an identity key.
    // Investigation Protocol: a missing field is not the literal name "ANON".
    // Migrate only from observed data at the anonymous-account boundary.
    #[serde(default)]
    pub display_name: String,
    pub profile: PlayerProfile,
    pub linked_identities: Vec<LinkedIdentity>,
    /// Account kind. Missing kind fields in older records deserialize as Human.
    /// Bots are accounts with `kind = Bot` — they have profiles, accumulate
    /// stats, and serve as the persistent identity pool for internal fillers.
    #[serde(default)]
    pub kind: AccountKind,
    /// BLAKE3 hash (hex) of the anonymous account secret. Only the hash is
    /// persisted; the plaintext is revealed to the client exactly once, when
    /// minted. Proves account ownership on `JoinWithAuth`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_secret_hash: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl PlayerAccount {
    /// Remove the anonymous ownership proof before returning an account from
    /// a public HTTP endpoint. The hash remains present in storage and in
    /// authenticated internal responses.
    pub fn without_auth_secret(mut self) -> Self {
        self.auth_secret_hash = None;
        self
    }
}

/// Distinguishes real players from persistent bot accounts. Bots are
/// full-fledged accounts (they have stats, profiles) — they just aren't
/// driven by a human. Used for stat filtering, leaderboards, display.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccountKind {
    #[default]
    Human,
    Bot,
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash, PartialEq, Eq)]
pub struct LinkedIdentity {
    pub provider: String, // "crazygames" for verified players or "bot" for server fillers
    pub external_id: String, // Stable unique ID from the platform
}

#[derive(Clone)]
pub struct PlayerDb {
    client: Client,
    pub crazygames_api_key: Option<String>,
    pub metadata_db: Option<std::sync::Arc<redb::Database>>,
}

fn utc_date_string() -> String {
    crate::events::utc_date_string()
}

impl PlayerDb {
    pub fn new(
        redis_url: &str,
        crazygames_api_key: Option<String>,
        metadata_db: Option<std::sync::Arc<redb::Database>>,
    ) -> Self {
        let client = Client::open(redis_url).expect("Failed to connect to Valkey/Redis");
        info!("Successfully initialized Valkey database connector client.");
        Self {
            client,
            crazygames_api_key,
            metadata_db,
        }
    }

    async fn get_connection(&self) -> Result<redis::aio::MultiplexedConnection, redis::RedisError> {
        self.client.get_multiplexed_async_connection().await
    }

    pub fn save_player_account_to_redb(&self, account: &PlayerAccount) {
        if let Some(ref db) = self.metadata_db
            && let Ok(write_txn) = db.begin_write()
        {
            if let Ok(mut table) = write_txn.open_table(PLAYERS_TABLE)
                && let Ok(json) = serde_json::to_string(account)
            {
                if let Err(error) = table.insert(account.id.as_str(), json.as_bytes()) {
                    error!("Failed to persist account {} to REDB: {error}", account.id);
                }
            }
            if let Ok(mut table) = write_txn.open_table(PUBLIC_PROFILES_TABLE) {
                let public_id = if account.public_id.is_empty() {
                    public_profile_id(&account.id)
                } else {
                    account.public_id.clone()
                };
                let index = PublicProfileIndex {
                    account_id: account.id.clone(),
                    public_id,
                    display_name: account.display_name.clone(),
                    kind: format!("{:?}", account.kind),
                    updated_at: account.updated_at,
                };
                if let Ok(json) = serde_json::to_vec(&index)
                    && let Err(error) = table.insert(index.public_id.as_str(), json.as_slice())
                {
                    error!("Failed to persist public profile index {}: {error}", account.id);
                }
            }
            if let Err(error) = write_txn.commit() {
                error!("Failed to commit account {} to REDB: {error}", account.id);
            }
        }
    }

    /// Persist one completed match and its per-player lookup keys. The match
    /// row is idempotent, so relay retries cannot create duplicate history.
    pub fn save_match_record(
        &self,
        record: &MatchRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Err("profile metadata database unavailable".into());
        };
        let write_txn = db.begin_write()?;
        let mut matches = write_txn.open_table(MATCHES_TABLE)?;
        let json = serde_json::to_vec(record)?;
        matches.insert(record.match_id.as_str(), json.as_slice())?;
        drop(matches);

        let mut index = write_txn.open_table(PLAYER_MATCH_INDEX_TABLE)?;
        for participant in &record.participants {
            let key = format!(
                "player:{}:{:020}:{}",
                participant.account_id, record.completed_at, record.match_id
            );
            index.insert(key.as_str(), record.match_id.as_bytes())?;
        }
        drop(index);
        write_txn.commit()?;
        Ok(())
    }

    pub fn save_season_rating(
        &self,
        rating: &SeasonRating,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Err("profile metadata database unavailable".into());
        };
        let key = format!(
            "rating:{}:{}:{}:{}",
            rating.account_id, rating.season_id, rating.queue, rating.mode
        );
        let write_txn = db.begin_write()?;
        let mut table = write_txn.open_table(SEASON_RATINGS_TABLE)?;
        let json = serde_json::to_vec(rating)?;
        table.insert(key.as_str(), json.as_slice())?;
        drop(table);
        write_txn.commit()?;
        Ok(())
    }

    fn load_season_rating(
        &self,
        account_id: &str,
        queue: &str,
        mode: &str,
    ) -> Result<Option<SeasonRating>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Ok(None);
        };
        let key = format!(
            "rating:{}:{}:{}:{}",
            account_id,
            crate::profile::CURRENT_SEASON_ID,
            queue,
            mode
        );
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(SEASON_RATINGS_TABLE)?;
        let Some(value) = table.get(key.as_str())? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_slice(value.value())?))
    }

    fn pair_update(winner: &mut SeasonRating, loser: &mut SeasonRating) {
        // Weng-Lin/OpenSkill update for one decisive comparison. FFA and team
        // results are reduced to pairwise comparisons, keeping the stored
        // ladder independent of client-provided UI values.
        let beta = 25.0 / 6.0;
        let c = (2.0 * beta * beta + winner.sigma * winner.sigma + loser.sigma * loser.sigma)
            .sqrt()
            .max(0.0001);
        let t = (winner.mu - loser.mu) / c;
        let normal = |x: f64| (-x * x / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt();
        let cdf = |x: f64| 1.0 / (1.0 + (-1.702 * x).exp());
        let v = normal(t) / cdf(t).max(1e-9);
        let w = v * (v + t);
        let winner_var = winner.sigma * winner.sigma;
        let loser_var = loser.sigma * loser.sigma;
        winner.mu += winner_var / c * v;
        loser.mu -= loser_var / c * v;
        winner.sigma *= (1.0 - winner_var / (c * c) * w).max(0.01).sqrt();
        loser.sigma *= (1.0 - loser_var / (c * c) * w).max(0.01).sqrt();
    }

    fn apply_ratings(
        &self,
        record: &mut MatchRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !record.rating_eligible {
            return Ok(());
        }
        let human_indexes = record
            .participants
            .iter()
            .enumerate()
            .filter(|(_, participant)| !participant.is_bot)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let mut ratings = human_indexes
            .iter()
            .map(|index| {
                let participant = &record.participants[*index];
                let rating = self
                    .load_season_rating(&participant.account_id, &record.queue, &record.mode)?
                    .unwrap_or(SeasonRating {
                        schema_version: 1,
                        account_id: participant.account_id.clone(),
                        season_id: record.season_id,
                        queue: record.queue.clone(),
                        mode: record.mode.clone(),
                        games_played: 0,
                        wins: 0,
                        mu: 25.0,
                        sigma: 8.333,
                        score: 0,
                        tier: "Provisional".to_string(),
                        division: None,
                        peak_score: 0,
                        placements_complete: false,
                        updated_at: 0,
                    });
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(rating)
            })
            .collect::<Result<Vec<_>, _>>()?;

        if record.mode == "HumansVsNations" && human_indexes.len() == 1 {
            let mut ai = SeasonRating {
                mu: 25.0,
                sigma: 1.0,
                ..ratings[0].clone()
            };
            ai.account_id.clear();
            let human_won = record.participants[human_indexes[0]].won;
            if human_won {
                Self::pair_update(&mut ratings[0], &mut ai);
            } else {
                Self::pair_update(&mut ai, &mut ratings[0]);
            }
        } else {
            for left in 0..ratings.len() {
                for right in left + 1..ratings.len() {
                    let left_won = record.participants[human_indexes[left]].won;
                    let right_won = record.participants[human_indexes[right]].won;
                    if left_won == right_won {
                        continue;
                    }
                    if left_won {
                        let (winner, loser) = ratings.split_at_mut(right);
                        Self::pair_update(&mut winner[left], &mut loser[0]);
                    } else {
                        let (winner, loser) = ratings.split_at_mut(right);
                        Self::pair_update(&mut loser[0], &mut winner[left]);
                    }
                }
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        for (index, rating) in ratings.iter_mut().enumerate() {
            let old_score = if rating.games_played == 0 {
                crate::profile::ladder_score(25.0, 8.333)
            } else {
                rating.score
            };
            let participant = &mut record.participants[human_indexes[index]];
            rating.games_played = rating.games_played.saturating_add(1);
            if participant.won {
                rating.wins = rating.wins.saturating_add(1);
            }
            rating.score = crate::profile::ladder_score(rating.mu, rating.sigma);
            rating.placements_complete = rating.games_played >= 5;
            let (tier, division) = crate::profile::tier_for_score(rating.score);
            rating.tier = if rating.placements_complete {
                tier.to_string()
            } else {
                "Provisional".to_string()
            };
            rating.division = if rating.placements_complete { division } else { None };
            rating.peak_score = rating.peak_score.max(rating.score);
            rating.updated_at = now;
            participant.rating_delta = Some(rating.score as i16 - old_score as i16);
            self.save_season_rating(rating)?;
        }
        Ok(())
    }

    pub fn save_season(
        &self,
        season: &SeasonRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Err("profile metadata database unavailable".into());
        };
        let key = format!("season:{:08}", season.id);
        let write_txn = db.begin_write()?;
        let mut table = write_txn.open_table(SEASONS_TABLE)?;
        let json = serde_json::to_vec(season)?;
        table.insert(key.as_str(), json.as_slice())?;
        drop(table);
        write_txn.commit()?;
        Ok(())
    }

    fn public_index(
        &self,
        public_id: &str,
    ) -> Result<Option<PublicProfileIndex>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Ok(None);
        };
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(PUBLIC_PROFILES_TABLE)?;
        let Some(value) = table.get(public_id)? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_slice(value.value())?))
    }

    fn load_match_record(
        &self,
        match_id: &str,
    ) -> Result<Option<MatchRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Ok(None);
        };
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(MATCHES_TABLE)?;
        let Some(value) = table.get(match_id)? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_slice(value.value())?))
    }

    fn match_history_for_account(
        &self,
        account_id: &str,
        offset: usize,
        limit: usize,
        queue: Option<&str>,
        mode: Option<&str>,
    ) -> Result<Vec<MatchRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Ok(Vec::new());
        };
        let prefix = format!("player:{account_id}:");
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(PLAYER_MATCH_INDEX_TABLE)?;
        let mut match_ids = Vec::new();
        for item in table.iter()? {
            let (key, value) = item?;
            if key.value().starts_with(&prefix)
                && let Ok(match_id) = std::str::from_utf8(value.value())
            {
                match_ids.push(match_id.to_string());
            }
        }
        drop(table);
        drop(read_txn);
        match_ids.sort_by(|left, right| right.cmp(left));
        let mut skipped = 0usize;
        let mut records = Vec::with_capacity(limit.min(match_ids.len().saturating_sub(offset)));
        for match_id in match_ids {
            if let Some(record) = self.load_match_record(&match_id)? {
                if queue.is_none_or(|value| value == record.queue)
                    && mode.is_none_or(|value| value == record.mode)
                {
                    if skipped < offset {
                        skipped += 1;
                        continue;
                    }
                    records.push(record);
                    if records.len() >= limit {
                        break;
                    }
                }
            }
        }
        Ok(records)
    }

    fn public_match_summary(
        account_id: &str,
        record: &MatchRecord,
    ) -> Option<PublicMatchSummary> {
        let participant = record
            .participants
            .iter()
            .find(|participant| participant.account_id == account_id)?;
        Some(PublicMatchSummary {
            match_id: record.match_id.clone(),
            completed_at: record.completed_at,
            queue: record.queue.clone(),
            mode: record.mode.clone(),
            map_name: record.map_name.clone(),
            duration_seconds: record.duration_seconds,
            placement: participant.placement,
            won: participant.won,
            leader: participant.leader.clone(),
            kills: participant.kills,
            deaths: participant.deaths,
            assists: participant.assists,
            players_defeated: participant.players_defeated,
            verified: record.verified,
            rating_delta: participant.rating_delta,
        })
    }

    fn public_summary(account: &PlayerAccount) -> PublicProfileSummary {
        let public_id = if account.public_id.is_empty() {
            public_profile_id(&account.id)
        } else {
            account.public_id.clone()
        };
        PublicProfileSummary {
            public_id: public_id.clone(),
            handle: public_handle(&account.display_name, &public_id),
            display_name: account.display_name.clone(),
            level: account.profile.level,
            matches_played: account.profile.matches_played,
            wins: account.profile.wins,
            win_rate: win_rate(account.profile.wins, account.profile.matches_played),
            kills: account.profile.kills,
            deaths: account.profile.deaths,
            assists: account.profile.assists,
        }
    }

    /// Public profile DTO. Exact XP and laurels remain in the authenticated
    /// menu bridge; this endpoint exposes level and gameplay statistics only.
    pub async fn public_profile(
        &self,
        public_id: &str,
    ) -> Result<Option<PublicProfileView>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(index) = self.public_index(public_id)? else {
            return Ok(None);
        };
        let mut con = self.get_connection().await?;
        let account = Self::load_account(&mut con, &index.account_id).await?;
        if account.kind == AccountKind::Bot
            || (account.public_id != public_id && !account.public_id.is_empty())
        {
            return Ok(None);
        }
        let mut leader_stats = account.profile.leader_stats.clone();
        for (leader, xp) in &account.profile.leader_xp {
            leader_stats.entry(leader.clone()).or_insert_with(|| LeaderCareerStats {
                xp: *xp,
                ..Default::default()
            });
        }
        let mut leaders = leader_stats
            .into_iter()
            .map(|(leader, stats)| PublicLeaderSummary {
                leader,
                matches_played: stats.matches_played,
                wins: stats.wins,
                win_rate: win_rate(stats.wins, stats.matches_played),
                kills: stats.kills,
                deaths: stats.deaths,
                assists: stats.assists,
                xp: stats.xp,
            })
            .collect::<Vec<_>>();
        leaders.sort_by(|left, right| right.xp.cmp(&left.xp));
        let recent_matches = self
            .match_history_for_account(&account.id, 0, 10, None, None)?
            .iter()
            .filter_map(|record| Self::public_match_summary(&account.id, record))
            .collect();
        let public_id = if account.public_id.is_empty() {
            public_profile_id(&account.id)
        } else {
            account.public_id.clone()
        };
        Ok(Some(PublicProfileView {
            public_id: public_id.clone(),
            handle: public_handle(&account.display_name, &public_id),
            display_name: account.display_name,
            level: account.profile.level,
            matches_played: account.profile.matches_played,
            wins: account.profile.wins,
            win_rate: win_rate(account.profile.wins, account.profile.matches_played),
            kills: account.profile.kills,
            deaths: account.profile.deaths,
            assists: account.profile.assists,
            players_defeated: account.profile.players_defeated,
            empires_defeated: account.profile.empires_defeated,
            tribes_defeated: account.profile.tribes_defeated,
            preferred_leader: account.profile.preferred_leader,
            leaders,
            recent_matches,
        }))
    }

    pub async fn search_public_profiles(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<PublicProfileSummary>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Ok(Vec::new());
        };
        let needle = query.trim().to_lowercase();
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(PUBLIC_PROFILES_TABLE)?;
        let mut indexes = Vec::new();
        for item in table.iter()? {
            let (_, value) = item?;
            let index: PublicProfileIndex = serde_json::from_slice(value.value())?;
            if index.kind == "Bot" {
                continue;
            }
            if needle.is_empty()
                || index.display_name.to_lowercase().contains(&needle)
                || index.public_id.to_lowercase().contains(&needle)
            {
                indexes.push(index);
            }
            if indexes.len() >= limit {
                break;
            }
        }
        drop(table);
        drop(read_txn);
        let mut con = self.get_connection().await?;
        let mut result = Vec::with_capacity(indexes.len());
        for index in indexes {
            if let Ok(account) = Self::load_account(&mut con, &index.account_id).await
                && account.kind != AccountKind::Bot
            {
                result.push(Self::public_summary(&account));
            }
        }
        Ok(result)
    }

    pub async fn public_match_history(
        &self,
        public_id: &str,
        offset: usize,
        limit: usize,
        queue: Option<&str>,
        mode: Option<&str>,
    ) -> Result<Option<Vec<PublicMatchSummary>>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(index) = self.public_index(public_id)? else {
            return Ok(None);
        };
        let mut con = self.get_connection().await?;
        let account = Self::load_account(&mut con, &index.account_id).await?;
        if account.kind == AccountKind::Bot
            || (account.public_id != public_id && !account.public_id.is_empty())
        {
            return Ok(None);
        }
        Ok(Some(
            self.match_history_for_account(
                &account.id,
                offset,
                limit.saturating_add(1),
                queue,
                mode,
            )?
                .iter()
                .filter_map(|record| Self::public_match_summary(&account.id, record))
                .collect(),
        ))
    }

    pub fn public_match_detail(
        &self,
        match_id: &str,
    ) -> Result<Option<PublicMatchDetail>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(record) = self.load_match_record(match_id)? else {
            return Ok(None);
        };
        let winner_public_id = record.winner_account_id.as_ref().and_then(|account_id| {
            record
                .participants
                .iter()
                .find(|participant| &participant.account_id == account_id)
                .map(|participant| participant.public_id.clone())
        });
        let participants = record
            .participants
            .iter()
            .map(|participant| PublicMatchParticipant {
                public_id: participant.public_id.clone(),
                handle: public_handle(&participant.display_name, &participant.public_id),
                is_bot: participant.is_bot,
                leader: participant.leader.clone(),
                team: participant.team.clone(),
                placement: participant.placement,
                won: participant.won,
                kills: participant.kills,
                deaths: participant.deaths,
                assists: participant.assists,
                players_defeated: participant.players_defeated,
                verified: record.verified,
                rating_delta: participant.rating_delta,
            })
            .collect();
        Ok(Some(PublicMatchDetail {
            match_id: record.match_id,
            completed_at: record.completed_at,
            queue: record.queue,
            mode: record.mode,
            map_name: record.map_name,
            duration_seconds: record.duration_seconds,
            winner_public_id,
            winning_team: record.winning_team,
            verified: record.verified,
            rating_eligible: record.rating_eligible,
            participants,
        }))
    }

    pub fn ensure_current_season(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Err("profile metadata database unavailable".into());
        };
        let key = format!("season:{:08}", crate::profile::CURRENT_SEASON_ID);
        let read_txn = db.begin_read()?;
        let exists = {
            let table = read_txn.open_table(SEASONS_TABLE)?;
            table.get(key.as_str())?.is_some()
        };
        drop(read_txn);
        if !exists {
            self.save_season(&SeasonRecord {
                id: crate::profile::CURRENT_SEASON_ID,
                name: crate::profile::CURRENT_SEASON_NAME.to_string(),
                status: "active".to_string(),
                starts_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                ends_at: None,
            })?;
        }
        Ok(())
    }

    pub fn seasons(
        &self,
    ) -> Result<Vec<SeasonRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Ok(Vec::new());
        };
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(SEASONS_TABLE)?;
        let mut seasons = Vec::new();
        for item in table.iter()? {
            let (_, value) = item?;
            seasons.push(serde_json::from_slice(value.value())?);
        }
        seasons.sort_by_key(|season: &SeasonRecord| season.id);
        Ok(seasons)
    }

    pub async fn public_ratings(
        &self,
        public_id: &str,
    ) -> Result<Option<Vec<PublicRatingView>>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(index) = self.public_index(public_id)? else {
            return Ok(None);
        };
        let mut con = self.get_connection().await?;
        let account = Self::load_account(&mut con, &index.account_id).await?;
        if account.kind == AccountKind::Bot
            || (account.public_id != public_id && !account.public_id.is_empty())
        {
            return Ok(None);
        }
        let Some(db) = &self.metadata_db else {
            return Ok(Some(Vec::new()));
        };
        let prefix = format!("rating:{}:", account.id);
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(SEASON_RATINGS_TABLE)?;
        let mut ratings = Vec::new();
        for item in table.iter()? {
            let (key, value) = item?;
            if key.value().starts_with(&prefix) {
                ratings.push(serde_json::from_slice::<SeasonRating>(value.value())?);
            }
        }
        drop(table);
        drop(read_txn);
        ratings.sort_by(|left, right| {
            right
                .season_id
                .cmp(&left.season_id)
                .then_with(|| left.queue.cmp(&right.queue))
                .then_with(|| left.mode.cmp(&right.mode))
        });
        Ok(Some(
            ratings
                .into_iter()
                .map(|rating| PublicRatingView {
                    season_id: rating.season_id,
                    season_name: if rating.season_id == crate::profile::CURRENT_SEASON_ID {
                        crate::profile::CURRENT_SEASON_NAME.to_string()
                    } else {
                        format!("Season {}", rating.season_id)
                    },
                    queue: rating.queue,
                    mode: rating.mode,
                    games_played: rating.games_played,
                    wins: rating.wins,
                    placements_complete: rating.placements_complete,
                    score: rating.score,
                    tier: rating.tier,
                    division: rating.division,
                    peak_score: rating.peak_score,
                })
                .collect(),
        ))
    }

    pub async fn public_leaderboard(
        &self,
        queue: &str,
        mode: &str,
        limit: usize,
    ) -> Result<Vec<PublicLeaderboardEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(db) = &self.metadata_db else {
            return Ok(Vec::new());
        };
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(SEASON_RATINGS_TABLE)?;
        let mut ratings = Vec::new();
        let marker = format!(":{}:{}:", crate::profile::CURRENT_SEASON_ID, queue);
        for item in table.iter()? {
            let (key, value) = item?;
            let key = key.value();
            if key.contains(&marker) && key.ends_with(&format!(":{mode}")) {
                ratings.push(serde_json::from_slice::<SeasonRating>(value.value())?);
            }
        }
        drop(table);
        drop(read_txn);
        let mut con = self.get_connection().await?;
        let mut entries = Vec::new();
        for rating in ratings {
            let Ok(account) = Self::load_account(&mut con, &rating.account_id).await else {
                continue;
            };
            if account.kind == AccountKind::Bot {
                continue;
            }
            let public_id = if account.public_id.is_empty() {
                public_profile_id(&account.id)
            } else {
                account.public_id.clone()
            };
            entries.push((rating, public_id, account.display_name));
        }
        entries.sort_by(|left, right| right.0.score.cmp(&left.0.score));
        Ok(entries
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(index, (rating, public_id, display_name))| PublicLeaderboardEntry {
                rank: index as u32 + 1,
                public_id: public_id.clone(),
                handle: public_handle(&display_name, &public_id),
                queue: rating.queue,
                mode: rating.mode,
                score: rating.score,
                tier: rating.tier,
                division: rating.division,
                games_played: rating.games_played,
                wins: rating.wins,
            })
            .collect())
    }

    /// Key format for looking up canonical account ID by platform identity
    fn identity_key(provider: &str, external_id: &str) -> String {
        format!("sow:player:identity:{}:{}", provider, external_id)
    }

    /// Key format for loading/saving full player account info
    fn account_key(account_id: &str) -> String {
        format!("sow:player:account:{}", account_id)
    }

    async fn load_account(
        con: &mut redis::aio::MultiplexedConnection,
        account_id: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let acc_key = Self::account_key(account_id);
        let acc_json: Option<String> = con.get(&acc_key).await?;
        let Some(acc_json) = acc_json else {
            return Err("Account not found".into());
        };
        Ok(serde_json::from_str(&acc_json)?)
    }

    async fn ensure_public_id(
        &self,
        account: PlayerAccount,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        if !account.public_id.is_empty() {
            return Ok(account);
        }
        let public_id = public_profile_id(&account.id);
        let mut con = self.get_connection().await?;
        let updated = Self::update_account_atomic(
            &mut con,
            &Self::account_key(&account.id),
            |account| {
                if account.public_id.is_empty() {
                    account.public_id = public_id.clone();
                    account.updated_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                }
            },
        )
        .await?;
        self.save_player_account_to_redb(&updated);
        Ok(updated)
    }

    async fn ensure_starting_leader(
        &self,
        account: PlayerAccount,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        if account.profile.preferred_leader.is_some() {
            return Ok(account);
        }
        let assigned = crate::commerce::leader_wire_id(crate::commerce::assigned_leader_for_account(
            &account.id,
            crate::commerce::current_rotation_period(),
        ))
        .to_string();
        let mut con = self.get_connection().await?;
        let updated = Self::update_account_atomic(
            &mut con,
            &Self::account_key(&account.id),
            |account| {
                if account.profile.preferred_leader.is_none() {
                    account.profile.preferred_leader = Some(assigned.clone());
                    account.updated_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                }
            },
        )
        .await?;
        self.save_player_account_to_redb(&updated);
        Ok(updated)
    }

    /// Replace one account only if the JSON read by this request is still the
    /// value in Valkey. This closes the rename/stats lost-update race without
    /// introducing another state store or a process-local lock.
    async fn update_account_atomic<F>(
        con: &mut redis::aio::MultiplexedConnection,
        acc_key: &str,
        mut mutate: F,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>>
    where
        F: FnMut(&mut PlayerAccount),
    {
        let compare_and_set = redis::Script::new(
            r#"if redis.call('GET', KEYS[1]) == ARGV[1] then
                    redis.call('SET', KEYS[1], ARGV[2])
                    return 1
                end
                return 0"#,
        );
        for _ in 0..5 {
            let Some(expected_json): Option<String> = con.get(acc_key).await? else {
                return Err("Account not found".into());
            };
            let mut account: PlayerAccount = serde_json::from_str(&expected_json)?;
            mutate(&mut account);
            let updated_json = serde_json::to_string(&account)?;
            let replaced: i32 = compare_and_set
                .key(acc_key)
                .arg(&expected_json)
                .arg(&updated_json)
                .invoke_async(con)
                .await?;
            if replaced == 1 {
                return Ok(account);
            }
        }
        Err("concurrent account update; retry".into())
    }

    async fn record_analytics(
        con: &mut redis::aio::MultiplexedConnection,
        account_id: &str,
        is_new: bool,
    ) -> Result<(), redis::RedisError> {
        if is_new {
            let _: () = con.pfadd(ANALYTICS_UNIQUE, account_id).await?;
        }
        let day_key = format!("{ANALYTICS_ACTIVE_PREFIX}{}", utc_date_string());
        let _: () = con.pfadd(&day_key, account_id).await?;
        let _: () = con.expire(&day_key, ANALYTICS_DAU_TTL_SECS).await?;
        let active_key = Self::daily_active_key(&utc_date_string());
        let _: () = con.sadd(&active_key, account_id).await?;
        let _: () = con.expire(&active_key, ANALYTICS_DAU_TTL_SECS).await?;
        Ok(())
    }

    async fn record_activation_in_connection(
        con: &mut redis::aio::MultiplexedConnection,
        account_id: &str,
        date: &str,
    ) -> Result<(), redis::RedisError> {
        let activated_key = format!("{ANALYTICS_ACTIVATED_PREFIX}{account_id}");
        let first_match: bool = con
            .set_nx(&activated_key, date)
            .await?;
        if first_match {
            let cohort_key = format!("{ANALYTICS_COHORT_PREFIX}{date}");
            let _: () = redis::pipe()
                .expire(&activated_key, ANALYTICS_RETENTION_TTL_SECS)
                .sadd(&cohort_key, account_id)
                .expire(&cohort_key, ANALYTICS_RETENTION_TTL_SECS)
                .query_async(con)
                .await?;
        }
        Ok(())
    }

    /// Record a client-originated product event in hot counters and exact
    /// daily activity sets. The JSONL sink remains the durable event source;
    /// these keys make DAU/funnel/retention queries cheap without a vendor.
    pub async fn record_product_event(
        &self,
        name: &str,
        account_id: Option<&str>,
    ) -> Result<(), redis::RedisError> {
        let mut con = self.get_connection().await?;
        let date = utc_date_string();
        let count_key = format!("{ANALYTICS_EVENT_COUNT_PREFIX}{date}:{name}");
        let _: u64 = con.incr(&count_key, 1_u64).await?;
        let _: () = con.expire(&count_key, ANALYTICS_RETENTION_TTL_SECS).await?;

        let Some(account_id) = account_id else {
            return Ok(());
        };
        let is_bot: i8 = con.sismember(BOT_POOL_KEY, account_id).await?;
        if is_bot == 1 {
            return Ok(());
        }

        let event_users_key = format!("{ANALYTICS_EVENT_USERS_PREFIX}{date}:{name}");
        let active_key = Self::daily_active_key(&date);
        let _: () = redis::pipe()
            .sadd(&event_users_key, account_id)
            .expire(&event_users_key, ANALYTICS_RETENTION_TTL_SECS)
            .sadd(&active_key, account_id)
            .expire(&active_key, ANALYTICS_RETENTION_TTL_SECS)
            .query_async(&mut con)
            .await?;

        if matches!(name, "match_ended" | "match_ended_client") {
            Self::record_activation_in_connection(&mut con, account_id, &date).await?;
        }
        Ok(())
    }

    /// Mark every human participant in an authoritative completed match as
    /// active and put first-time completers into the activation cohort.
    pub async fn record_match_activation(
        &self,
        account_ids: &[String],
    ) -> Result<(), redis::RedisError> {
        let mut con = self.get_connection().await?;
        let date = utc_date_string();
        for account_id in account_ids {
            let is_bot: i8 = con.sismember(BOT_POOL_KEY, account_id).await?;
            if is_bot == 1 {
                continue;
            }
            let active_key = Self::daily_active_key(&date);
            let _: () = redis::pipe()
                .sadd(&active_key, account_id)
                .expire(&active_key, ANALYTICS_RETENTION_TTL_SECS)
                .query_async(&mut con)
                .await?;
            Self::record_activation_in_connection(&mut con, account_id, &date).await?;
        }
        Ok(())
    }

    /// Return a compact, authenticated-only analytics snapshot for operators.
    /// Retention uses exact daily sets, not HLL intersections.
    pub async fn analytics_summary(
        &self,
        requested_days: u32,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let days = requested_days.clamp(1, 90) as i64;
        let today = utc_date_string();
        let names = [
            "landing_visit",
            "play_now_click",
            "shell_loaded",
            "matchmaking_joined",
            "match_started",
            "match_ended",
        ];
        let mut con = self.get_connection().await?;
        let mut daily = Vec::new();
        let mut funnel = serde_json::Map::new();
        for offset in 0..days {
            let date = crate::events::shift_date(&today, -offset).ok_or("invalid UTC date")?;
            let mut counts = serde_json::Map::new();
            for name in names {
                let key = format!("{ANALYTICS_EVENT_COUNT_PREFIX}{date}:{name}");
                // GET on a missing counter returns nil, which is a normal
                // zero-count day—not a Redis failure.
                let count: u64 = con.get::<_, Option<u64>>(&key).await?.unwrap_or(0);
                counts.insert(name.to_string(), serde_json::json!(count));
                let total = funnel
                    .entry(name.to_string())
                    .or_insert_with(|| serde_json::json!(0_u64));
                *total = serde_json::json!(total.as_u64().unwrap_or(0).saturating_add(count));
            }
            let active_key = Self::daily_active_key(&date);
            let active: usize = con.scard(&active_key).await?;
            daily.push(serde_json::json!({
                "date": date,
                "active_users": active,
                "events": counts,
            }));
        }

        let mut eligible_cohorts = 0_u64;
        let mut d1_returned = 0_u64;
        let mut d7_returned = 0_u64;
        for offset in 7..days {
            let cohort_date = crate::events::shift_date(&today, -offset).ok_or("invalid cohort date")?;
            let members: Vec<String> = con
                .smembers(format!("{ANALYTICS_COHORT_PREFIX}{cohort_date}"))
                .await?;
            if members.is_empty() {
                continue;
            }
            eligible_cohorts += members.len() as u64;
            let day_one = crate::events::shift_date(&cohort_date, 1).ok_or("invalid D1 date")?;
            let day_seven = crate::events::shift_date(&cohort_date, 7).ok_or("invalid D7 date")?;
            for account_id in members {
                if con
                    .sismember::<_, _, bool>(Self::daily_active_key(&day_one), &account_id)
                    .await?
                {
                    d1_returned += 1;
                }
                if con
                    .sismember::<_, _, bool>(Self::daily_active_key(&day_seven), &account_id)
                    .await?
                {
                    d7_returned += 1;
                }
            }
        }
        Ok(serde_json::json!({
            "generated_at": today,
            "days": days,
            "funnel": funnel,
            "daily": daily,
            "retention": {
                "eligible_activated_players": eligible_cohorts,
                "d1_returned": d1_returned,
                "d7_returned": d7_returned,
            }
        }))
    }

    /// Key holding the SET of account_ids active on a UTC date. Exact
    /// membership (unlike the DAU HyperLogLog) is what makes D1/Dn retention
    /// computable.
    fn daily_active_key(date: &str) -> String {
        format!("sow:active:{date}")
    }

    /// Membership check against the bot pool index — used to exclude synthetic
    /// players from analytics ingestion and human counters.
    pub async fn is_bot_account_checked(
        &self,
        account_id: &str,
    ) -> Result<bool, redis::RedisError> {
        let mut con = self.get_connection().await?;
        Ok(con.sismember::<_, _, i8>(BOT_POOL_KEY, account_id).await? == 1)
    }

    pub async fn is_bot_account(&self, account_id: &str) -> bool {
        match self.is_bot_account_checked(account_id).await {
            Ok(is_bot) => is_bot,
            Err(error) => {
                error!("Bot-pool membership lookup failed for {account_id}: {error}");
                false
            }
        }
    }

    /// Count non-bot account_ids in a match roster.
    pub async fn count_human_players(&self, player_ids: &[String]) -> usize {
        let Ok(mut con) = self.get_connection().await else {
            return 0;
        };
        let mut humans = 0usize;
        for id in player_ids {
            let is_bot: i8 = match con.sismember(BOT_POOL_KEY, id).await {
                Ok(value) => value,
                Err(error) => {
                    error!("bot-pool lookup failed while counting players: {error}");
                    continue;
                }
            };
            if is_bot == 0 {
                humans += 1;
            }
        }
        humans
    }

    /// Read a match's registered roster and exit order before finalize deletes them.
    pub async fn match_participants(
        &self,
        match_id: &str,
    ) -> Result<(Vec<String>, Vec<String>), Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let players_json: Option<String> = con
            .get(format!("sow:match:{match_id}:players"))
            .await?;
        let exits: Vec<String> = con
            .lrange(format!("sow:match:{match_id}:exits"), 0, -1)
            .await?;
        let players = players_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        Ok((players, exits))
    }

    /// Record exact daily activity for an account (retention cohorts).
    pub async fn mark_daily_activity(&self, account_id: &str) {
        let Ok(mut con) = self.get_connection().await else {
            return;
        };
        let date = utc_date_string();
        let key = Self::daily_active_key(&date);
        let _: Result<(), _> = redis::pipe()
            .sadd(&key, account_id)
            .expire(&key, ANALYTICS_DAU_TTL_SECS)
            .query_async(&mut con)
            .await;
    }

    /// Get or create player account by platform identity
    pub async fn get_or_create(
        &self,
        provider: String,
        external_id: String,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let id_key = Self::identity_key(&provider, &external_id);

        // 1. Try to find existing account ID mapped to this identity
        if let Some(account_id) = con.get::<_, Option<String>>(&id_key).await?
            && let Ok(account) = Self::load_account(&mut con, &account_id).await
        {
            let _: () = Self::record_analytics(&mut con, &account.id, false).await?;
            let account = self.ensure_public_id(account).await?;
            return self.ensure_starting_leader(account).await;
        }

        // 2. Not found, create new stable account ID and register
        let random_id = format!("{:032x}", rand::random::<u128>());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let identity = LinkedIdentity {
            provider,
            external_id,
        };

        // Bot identities (provider == "bot") are marked at creation. All
        // Provider identities are human accounts unless explicitly seeded as bots.
        let kind = if identity.provider == "bot" {
            AccountKind::Bot
        } else {
            AccountKind::Human
        };

        let new_account = PlayerAccount {
            id: random_id.clone(),
            public_id: public_profile_id(&random_id),
            display_name: generated_display_name(),
            profile: PlayerProfile::for_account(&random_id),
            linked_identities: vec![identity.clone()],
            kind,
            auth_secret_hash: None,
            created_at: now,
            updated_at: now,
        };

        let acc_key = Self::account_key(&random_id);
        let acc_json = serde_json::to_string(&new_account)?;

        // Store account and map identity atomically
        redis::pipe()
            .set(&acc_key, acc_json)
            .set(&id_key, &random_id)
            .query_async::<()>(&mut con)
            .await?;

        let _: () = Self::record_analytics(&mut con, &random_id, true).await?;

        info!(
            "Created new account {} in Valkey for identity {:?}/{:?}",
            new_account.id, identity.provider, identity.external_id
        );
        self.save_player_account_to_redb(&new_account);
        Ok(new_account)
    }

    /// Migrate legacy accounts once at database boot. The scan is bounded by
    /// Valkey's cursor API and only writes accounts missing their public ID;
    /// existing profile aggregates are never rewritten.
    pub async fn backfill_public_profiles(
        &self,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let mut cursor = 0_u64;
        let mut migrated = 0_usize;
        loop {
            let (next, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg("sow:player:account:*")
                .arg("COUNT")
                .arg(256)
                .query_async(&mut con)
                .await?;
            for key in keys {
                let Some(json): Option<String> = con.get(&key).await? else {
                    continue;
                };
                let account: PlayerAccount = match serde_json::from_str(&json) {
                    Ok(account) => account,
                    Err(error) => {
                        error!("Skipping malformed account during profile migration {key}: {error}");
                        continue;
                    }
                };
                if account.public_id.is_empty()
                    || account.display_name.trim().is_empty()
                    || account.display_name == "ANON"
                {
                    let public_id = public_profile_id(&account.id);
                    let migrated_name = if account.display_name.trim().is_empty()
                        || account.display_name == "ANON"
                    {
                        Some(generated_display_name())
                    } else {
                        None
                    };
                    let updated = Self::update_account_atomic(&mut con, &key, |account| {
                        if account.public_id.is_empty() {
                            account.public_id = public_id.clone();
                        }
                        if let Some(name) = migrated_name.as_deref()
                            && (account.display_name.trim().is_empty()
                                || account.display_name == "ANON")
                        {
                            account.display_name = name.to_string();
                        }
                        account.updated_at = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                    })
                    .await?;
                    self.save_player_account_to_redb(&updated);
                    migrated += 1;
                } else {
                    self.save_player_account_to_redb(&account);
                }
            }
            cursor = next;
            if cursor == 0 {
                break;
            }
        }
        Ok(migrated)
    }

    /// Load or create an anonymous account using the canonical account ID.
    /// New anonymous accounts do not create a provider identity index.
    pub async fn get_or_create_anonymous(
        &self,
        account_id: Option<&str>,
        requested_display_name: Option<&str>,
        account_auth_secret: Option<&str>,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;

        if let Some(account_id) = account_id.map(str::trim).filter(|id| !id.is_empty()) {
            if !is_valid_account_id(account_id) {
                return Err("account_id must be exactly 32 hexadecimal characters".into());
            }
            let account = Self::load_account(&mut con, account_id).await?;
            if !account.linked_identities.is_empty() {
                return Err("account is not anonymous".into());
            }
            if let Some(stored_hash) = account.auth_secret_hash.as_deref()
                && account_auth_secret.is_none_or(|secret| {
                    blake3::hash(secret.as_bytes()).to_hex().to_string() != stored_hash
                })
            {
                return Err("invalid secret".into());
            }
            let account = self.ensure_starting_leader(self.ensure_public_id(account).await?).await?;
            if account.display_name.trim().is_empty() || account.display_name == "ANON" {
                let migrated_name = requested_display_name
                    .map(normalize_display_name)
                    .transpose()
                    .map_err(|error| error.to_string())?
                    .unwrap_or_else(generated_display_name);
                let acc_key = Self::account_key(account_id);
                let account = Self::update_account_atomic(&mut con, &acc_key, |account| {
                    if account.display_name.trim().is_empty() || account.display_name == "ANON" {
                        account.display_name = migrated_name.clone();
                        account.updated_at = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                    }
                })
                .await?;
                self.save_player_account_to_redb(&account);
                return Ok(account);
            }
            return Ok(account);
        }

        let display_name = requested_display_name
            .map(normalize_display_name)
            .transpose()
            .map_err(|error| error.to_string())?
            .unwrap_or_else(generated_display_name);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        loop {
            let random_id = format!("{:032x}", rand::random::<u128>());
            let account = PlayerAccount {
                id: random_id.clone(),
                public_id: public_profile_id(&random_id),
                display_name: display_name.clone(),
                profile: PlayerProfile::for_account(&random_id),
                linked_identities: Vec::new(),
                kind: AccountKind::Human,
                auth_secret_hash: None,
                created_at: now,
                updated_at: now,
            };
            let acc_key = Self::account_key(&random_id);
            let acc_json = serde_json::to_string(&account)?;

            // SETNX makes the canonical account key collision-safe without a
            // second identity mapping or a distributed lock.
            if con.set_nx::<_, _, bool>(&acc_key, acc_json).await? {
                let _: () = Self::record_analytics(&mut con, &random_id, true).await?;
                self.save_player_account_to_redb(&account);
                info!("Created anonymous account {}", account.id);
                return Ok(account);
            }
        }
    }

    /// Mint an anonymous account secret if the account has none yet. Persists
    /// only the BLAKE3 hash; returns the plaintext exactly once (the caller
    /// hands it to the client, which stores it as its ownership proof).
    /// Idempotent: an account that already has a secret returns `None`.
    pub async fn ensure_auth_secret(
        &self,
        account_id: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let acc_key = Self::account_key(account_id);
        let plain = format!("{:032x}{:032x}", rand::random::<u128>(), rand::random::<u128>());
        let hash = blake3::hash(plain.as_bytes()).to_hex().to_string();
        let mut revealed: Option<String> = None;
        let account = Self::update_account_atomic(&mut con, &acc_key, |account| {
            if account.auth_secret_hash.is_none() {
                account.auth_secret_hash = Some(hash.clone());
                account.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                revealed = Some(plain.clone());
            }
        })
        .await?;
        if revealed.is_some() {
            self.save_player_account_to_redb(&account);
        }
        Ok(revealed)
    }

    /// Verify an anonymous ownership proof: the presented secret must hash to
    /// the stored `auth_secret_hash`. Returns the account id on success.
    pub async fn verify_anonymous_secret(
        &self,
        account_id: &str,
        secret: &str,
    ) -> Result<String, String> {
        if !is_valid_account_id(account_id) {
            return Err("account_id must be exactly 32 hexadecimal characters".to_string());
        }
        let mut con = self
            .get_connection()
            .await
            .map_err(|e| format!("database unavailable: {e}"))?;
        let account = Self::load_account(&mut con, account_id)
            .await
            .map_err(|_| "account not found".to_string())?;
        match account.auth_secret_hash.as_deref() {
            Some(stored) if blake3::hash(secret.as_bytes()).to_hex().to_string() == stored => {
                Ok(account_id.to_string())
            }
            Some(_) => Err("invalid secret".to_string()),
            None => Err("account has no secret; fetch /profile/anonymous to mint one".to_string()),
        }
    }

    pub async fn resolve_leader_for_account(
        &self,
        account_id: &str,
        requested_leader: Option<&str>,
    ) -> Result<crate::commerce::LeaderResolution, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let account = Self::load_account(&mut con, account_id).await?;
        Ok(crate::commerce::resolve_leader(
            requested_leader,
            account_id,
            &account.profile.owned_leaders,
            crate::commerce::current_rotation_period(),
        ))
    }

    pub fn account_id_for_public_id(
        &self,
        public_id: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        self.public_index(public_id)
            .map(|index| index.map(|index| index.account_id))
    }

    pub async fn unlock_leader_with_laurels(
        &self,
        account_id: &str,
        requested_leader: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let leader = crate::commerce::leader_from_id(requested_leader)
            .ok_or("unknown leader")?;
        let leader_id = crate::commerce::leader_id(leader).to_string();
        let period = crate::commerce::current_rotation_period();
        let mut con = self.get_connection().await?;
        let mut failure = None;
        let account = Self::update_account_atomic(
            &mut con,
            &Self::account_key(account_id),
            |account| {
                if account.profile.owned_leaders.contains(&leader_id) {
                    failure = Some("leader already owned");
                } else if crate::commerce::leader_available(
                    leader,
                    &account.profile.owned_leaders,
                    period,
                ) {
                    failure = Some("leader is currently free in rotation");
                } else if account.profile.laurels < crate::commerce::LEADER_UNLOCK_COST_LAURELS {
                    failure = Some("insufficient laurels");
                } else {
                    account.profile.laurels -= crate::commerce::LEADER_UNLOCK_COST_LAURELS;
                    account.profile.owned_leaders.insert(leader_id.clone());
                    account.updated_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                }
            },
        )
        .await?;
        if let Some(error) = failure {
            return Err(error.into());
        }
        self.save_player_account_to_redb(&account);
        Ok(account)
    }

    pub async fn unlock_skin_with_gems(
        &self,
        account_id: &str,
        requested_skin: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let skin = crate::commerce::skin_by_id(requested_skin).ok_or("unknown skin")?;
        let mut failure = None;
        let mut con = self.get_connection().await?;
        let account = Self::update_account_atomic(
            &mut con,
            &Self::account_key(account_id),
            |account| {
                if account.profile.owned_skins.contains(&skin.id) {
                    failure = Some("skin already owned");
                } else if account.profile.gems < skin.cost_gems {
                    failure = Some("insufficient gems");
                } else {
                    account.profile.gems -= skin.cost_gems;
                    account.profile.owned_skins.insert(skin.id.clone());
                    account.updated_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                }
            },
        )
        .await?;
        if let Some(error) = failure {
            return Err(error.into());
        }
        self.save_player_account_to_redb(&account);
        Ok(account)
    }

    pub async fn equip_skin(
        &self,
        account_id: &str,
        requested_skin: Option<&str>,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let requested_skin = requested_skin.map(str::trim).filter(|id| !id.is_empty());
        if let Some(skin_id) = requested_skin {
            if crate::commerce::skin_by_id(skin_id).is_none() {
                return Err("unknown skin".into());
            }
        }
        let mut failure = None;
        let mut con = self.get_connection().await?;
        let account = Self::update_account_atomic(
            &mut con,
            &Self::account_key(account_id),
            |account| {
                if let Some(skin_id) = requested_skin {
                    if !account.profile.owned_skins.contains(skin_id) {
                        failure = Some("skin is not owned");
                        return;
                    }
                    account.profile.selected_skin = Some(skin_id.to_string());
                } else {
                    account.profile.selected_skin = None;
                }
                account.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
            },
        )
        .await?;
        if let Some(error) = failure {
            return Err(error.into());
        }
        self.save_player_account_to_redb(&account);
        Ok(account)
    }

    pub async fn grant_revenuecat_gems(
        &self,
        event_id: &str,
        purchase_user_id: &str,
        product_id: &str,
    ) -> Result<(PlayerAccount, bool), Box<dyn std::error::Error + Send + Sync>> {
        let account_id = if is_valid_account_id(purchase_user_id) {
            purchase_user_id.to_string()
        } else {
            self.account_id_for_public_id(purchase_user_id)?
                .ok_or("app_user_id must be a known public profile ID")?
        };
        let event_id = event_id.trim();
        if event_id.is_empty() || event_id.len() > 256 {
            return Err("RevenueCat event id is invalid".into());
        }
        let product_id = product_id.trim();
        let gems = crate::commerce::gem_amount_for_product(product_id)
            .ok_or("unknown RevenueCat product")?;
        let mut con = self.get_connection().await?;
        let mut granted = false;
        let account = Self::update_account_atomic(
            &mut con,
            &Self::account_key(&account_id),
            |account| {
                if account
                    .profile
                    .processed_revenuecat_events
                    .contains(event_id)
                {
                    return;
                }
                account
                    .profile
                    .processed_revenuecat_events
                    .insert(event_id.to_string());
                account.profile.gems = account.profile.gems.saturating_add(gems);
                account.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                granted = true;
            },
        )
        .await?;
        if granted {
            self.save_player_account_to_redb(&account);
        }
        Ok((account, granted))
    }

    /// Seed the persistent bot-account pool. For each external_id, performs
    /// a get-or-create with `provider = "bot"` (so created accounts carry
    /// `kind = Bot`). Idempotent — re-running with the same external_ids
    /// returns the existing account_ids without duplication. Also maintains
    /// the `sow:bot:pool` SET as an index of all bot account_ids for
    /// analytics / debugging.
    ///
    /// Bot display names remain server-managed; the account record still has a
    /// canonical display_name field for schema consistency.
    pub async fn seed_bot_pool(
        &self,
        external_ids: Vec<String>,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut account_ids = Vec::with_capacity(external_ids.len());

        for external_id in external_ids {
            let id_key = Self::identity_key("bot", &external_id);

            // 1. Fast path: identity already mapped.
            if let Some(account_id) = con.get::<_, Option<String>>(&id_key).await? {
                account_ids.push(account_id);
                continue;
            }

            // 2. Create new bot account.
            let random_id = format!("{:032x}", rand::random::<u128>());
            let identity = LinkedIdentity {
                provider: "bot".to_string(),
                external_id: external_id.clone(),
            };
            let account = PlayerAccount {
                id: random_id.clone(),
                public_id: public_profile_id(&random_id),
                display_name: generated_display_name(),
                profile: PlayerProfile::for_account(&random_id),
                linked_identities: vec![identity.clone()],
                kind: AccountKind::Bot,
                auth_secret_hash: None,
                created_at: now,
                updated_at: now,
            };
            let acc_key = Self::account_key(&random_id);
            let acc_json = serde_json::to_string(&account)?;

            redis::pipe()
                .set(&acc_key, acc_json)
                .set(&id_key, &random_id)
                .sadd(BOT_POOL_KEY, &random_id)
                .query_async::<()>(&mut con)
                .await?;

            self.save_player_account_to_redb(&account);
            account_ids.push(random_id);
        }

        info!(
            "[bot-pool] seed resolved {} bot accounts",
            account_ids.len()
        );
        Ok(account_ids)
    }

    /// Update player profile stats
    pub async fn update_profile(
        &self,
        account_id: &str,
        profile: PlayerProfile,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let acc_key = Self::account_key(account_id);

        if con.exists(&acc_key).await? {
            let account = Self::update_account_atomic(&mut con, &acc_key, |account| {
                account.profile = profile.clone();
                account.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
            })
            .await?;
            self.save_player_account_to_redb(&account);
            Ok(account)
        } else {
            Err("Account not found".into())
        }
    }

    /// Rename an anonymous account. The account ID is the bearer identity;
    /// the mutable display name is never treated as an account key.
    pub async fn update_anonymous_display_name(
        &self,
        account_id: &str,
        display_name: &str,
        auth_secret: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        if !is_valid_account_id(account_id) {
            return Err("account_id must be exactly 32 hexadecimal characters".into());
        }
        let display_name = normalize_display_name(display_name).map_err(str::to_string)?;
        let mut con = self.get_connection().await?;
        let acc_key = Self::account_key(account_id);
        let Some(acc_json) = con.get::<_, Option<String>>(&acc_key).await? else {
            return Err("Account not found".into());
        };
        let account: PlayerAccount = serde_json::from_str(&acc_json)?;
        if !account.linked_identities.is_empty() {
            return Err("account is not anonymous".into());
        }
        let provided_hash = blake3::hash(auth_secret.as_bytes()).to_hex().to_string();
        if account.auth_secret_hash.as_deref() != Some(provided_hash.as_str()) {
            return Err("invalid secret".into());
        }
        let account = Self::update_account_atomic(&mut con, &acc_key, |account| {
            account.display_name = display_name.clone();
            account.updated_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        })
        .await?;
        self.save_player_account_to_redb(&account);
        Ok(account)
    }

    /// Record a match outcome with KDA stats from relay-logged client submissions.
    pub async fn record_match_outcome_with_kda(
        &self,
        account_id: &str,
        won: bool,
        kda: MatchOutcomeKda,
        preferred_leader: Option<String>,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let acc_key = Self::account_key(account_id);

        if con.exists(&acc_key).await? {
            let account = Self::update_account_atomic(&mut con, &acc_key, |account| {
                let leader = preferred_leader
                    .clone()
                    .or_else(|| kda.leader.clone());
                account.profile.record_match_with_leader(
                    won,
                    kda.defeats,
                    kda.kills,
                    kda.deaths,
                    kda.assists,
                    leader.as_deref(),
                );
                if let Some(leader) = leader.as_deref().and_then(crate::commerce::leader_from_id) {
                    account.profile.preferred_leader =
                        Some(crate::commerce::leader_wire_id(leader).to_string());
                }
                account.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
            })
            .await?;
            self.save_player_account_to_redb(&account);
            Ok(account)
        } else {
            Err("Account not found".into())
        }
    }

    /// Complete the offline tutorial once and grant its fixed onboarding reward.
    /// The account secret is verified by the HTTP handler before this method runs.
    pub async fn complete_anonymous_tutorial(
        &self,
        account_id: &str,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let acc_key = Self::account_key(account_id);
        let account = Self::update_account_atomic(&mut con, &acc_key, |account| {
            if account.profile.intro_completed {
                return;
            }
            account.profile.intro_completed = true;
            let leader = account
                .profile
                .preferred_leader
                .as_deref()
                .and_then(crate::commerce::leader_from_id)
                .unwrap_or(crate::leaders::Leader::Boudica);
            account.profile.preferred_leader =
                Some(crate::commerce::leader_wire_id(leader).to_string());
            account.profile.apply_reward(
                leader.name(),
                crate::rewards::calculate(crate::rewards::RewardInput {
                    tutorial: true,
                    ..Default::default()
                }),
            );
            account.updated_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        })
        .await?;
        self.save_player_account_to_redb(&account);
        Ok(account)
    }

    /// Register expected players for an upcoming match.
    pub async fn register_match_start(
        &self,
        match_id: &str,
        player_ids: &[String],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let key = format!("sow:match:{match_id}:players");
        let players_json = serde_json::to_string(player_ids)?;
        redis::pipe()
            .set(&key, players_json)
            .expire(&key, 3600)
            .query_async::<()>(&mut con)
            .await?;
        info!(
            "Registered match {match_id} with {} players",
            player_ids.len()
        );
        Ok(())
    }

    /// Finalize a match from relay-logged exit order in Valkey. The optional
    /// lobby snapshot is retained in the durable public history record.
    pub async fn finalize_match(
        &self,
        match_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.finalize_match_with_lobby(match_id, None).await
    }

    pub async fn finalize_match_with_lobby(
        &self,
        match_id: &str,
        lobby_json: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let finalized_key = format!("sow:match:{match_id}:finalized");
        if con.exists(&finalized_key).await? {
            return Ok(());
        }
        if self.load_match_record(match_id)?.is_some() {
            let _: () = con.set(&finalized_key, "1").await?;
            return Ok(());
        }

        let players_key = format!("sow:match:{match_id}:players");
        let exits_key = format!("sow:match:{match_id}:exits");
        let players_json: Option<String> = con.get(&players_key).await?;
        let Some(players_json) = players_json else {
            // No account players registered (e.g. bot-only matches): the
            // replay was already archived by the handler; there is no stats
            // state to finalize, so treat it as a clean success.
            return Ok(());
        };
        let players: Vec<String> = serde_json::from_str(&players_json)?;
        if players.is_empty() {
            return Ok(());
        }

        let exits: Vec<String> = con.lrange(&exits_key, 0, -1).await?;
        let exit_winner = players
            .iter()
            .find(|p| !exits.contains(p))
            .cloned()
            .or_else(|| exits.last().cloned());

        let lobby_value = lobby_json
            .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
            .unwrap_or_default();
        let lobby_players = lobby_value
            .get("players")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let queue = lobby_value
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .filter(|kind| !kind.is_empty())
            .unwrap_or("Matchmaking")
            .to_string();
        let config = lobby_value.get("config");
        let mode = config
            .and_then(|value| value.get("game_mode"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("FFA")
            .to_string();
        let map_name = config
            .and_then(|value| value.get("map_name"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("world")
            .to_string();
        let raw_player = |account_id: &str| {
            lobby_players.iter().find(|player| {
                player
                    .get("database_account_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(account_id)
            })
        };
        let team_for = |account_id: &str| {
            raw_player(account_id)
                .and_then(|player| player.get("team"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        };
        let account_for_player_id = |player_id: u16| {
            lobby_players
                .iter()
                .find(|player| {
                    player
                        .get("player_id")
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|value| u16::try_from(value).ok())
                        == Some(player_id)
                })
                .and_then(|player| player.get("database_account_id"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        };
        let expected_humans = players
            .iter()
            .filter(|account_id| {
                !raw_player(account_id)
                    .and_then(|player| player.get("is_internal"))
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
            })
            .count();
        let mut reports = Vec::new();
        for account_id in &players {
            let is_internal = raw_player(account_id)
                .and_then(|player| player.get("is_internal"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if is_internal {
                continue;
            }
            let report_key = format!("sow:match:{match_id}:report:{account_id}");
            let report: std::collections::HashMap<String, String> =
                con.hgetall(&report_key).await?;
            let Some(winner_player_id) = report
                .get("winner_player_id")
                .and_then(|value| value.parse::<u16>().ok())
            else {
                continue;
            };
            let tick = report
                .get("tick")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            if tick == 0 {
                continue;
            }
            let winning_team = report
                .get("winning_team")
                .filter(|value| !value.is_empty())
                .cloned();
            reports.push((account_id.clone(), winner_player_id, winning_team));
        }
        let reports_consistent = expected_humans > 0
            && reports.len() == expected_humans
            && reports.iter().all(|(_, winner_player_id, winning_team)| {
                reports
                    .first()
                    .is_some_and(|(_, first_winner, first_team)| {
                        first_winner == winner_player_id && first_team == winning_team
                    })
            });
        let reported_winner = reports_consistent
            .then(|| reports[0].1)
            .and_then(account_for_player_id);
        let winner = reported_winner.or(exit_winner);
        let winning_team = if reports_consistent {
            reports[0].2.clone()
        } else {
            winner.as_deref().and_then(team_for)
        };
        let completed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let opponent_count = players.len().saturating_sub(1) as u32;
        let mut participant_records = Vec::with_capacity(players.len());
        for account_id in &players {
            let raw = raw_player(account_id);
            let raw_leader = raw
                .and_then(|player| player.get("leader"))
                .and_then(serde_json::Value::as_str)
                .and_then(crate::rewards::canonical_leader_name);
            let stats_key = format!("sow:match:{match_id}:stats:{account_id}");
            let stats: Option<std::collections::HashMap<String, String>> =
                con.hgetall(&stats_key).await?;
            let team = team_for(account_id);
            let won_by_player = winner.as_ref() == Some(account_id);
            let won = winning_team
                .as_ref()
                .map_or(won_by_player, |winning| team.as_ref() == Some(winning));

            let (defeats, kills, deaths, assists, leader) = if let Some(map) = stats {
                let parse = |k: &str| map.get(k).and_then(|v| v.parse().ok()).unwrap_or(0);
                (
                    SessionDefeats {
                        players: parse("players_defeated"),
                        empires: parse("empires_defeated"),
                        tribes: parse("tribes_defeated"),
                    },
                    parse("kills"),
                    parse("deaths"),
                    parse("assists"),
                    map.get("leader")
                        .and_then(|value| crate::rewards::canonical_leader_name(value)),
                )
            } else {
                (
                    SessionDefeats {
                        players: if won { opponent_count } else { 0 },
                        empires: 0,
                        tribes: 0,
                    },
                    0,
                    0,
                    0,
                    None,
                )
            };
            let leader = leader.or(raw_leader);
            let placement = exits
                .iter()
                .position(|exit| exit == account_id)
                .map(|position| (exits.len().saturating_sub(position)) as u16)
                .unwrap_or(1);
            let account = Self::load_account(&mut con, account_id).await?;
            let public_id = if account.public_id.is_empty() {
                public_profile_id(account_id)
            } else {
                account.public_id.clone()
            };
            let reward = crate::rewards::calculate(crate::rewards::RewardInput {
                won,
                players_defeated: defeats.players,
                empires_defeated: defeats.empires,
                tribes_defeated: defeats.tribes,
                kills,
                assists,
                ..Default::default()
            });
            participant_records.push(crate::profile::MatchParticipantRecord {
                account_id: account_id.clone(),
                public_id,
                display_name: account.display_name,
                is_bot: account.kind == AccountKind::Bot,
                leader: leader.clone(),
                team,
                placement,
                won,
                kills,
                deaths,
                assists,
                players_defeated: defeats.players,
                empires_defeated: defeats.empires,
                tribes_defeated: defeats.tribes,
                xp: reward.xp,
                leader_xp: reward.leader_xp,
                laurels: reward.laurels,
                rating_delta: None,
            });

            match self
                .record_match_outcome_with_kda(
                    account_id,
                    won,
                    MatchOutcomeKda {
                        defeats,
                        kills,
                        deaths,
                        assists,
                        leader,
                    },
                    None,
                )
                .await
            {
                Ok(account) => {
                    self.submit_crazygames_score(&account).await;
                }
                Err(e) => {
                    error!("Failed to record outcome for {account_id}: {e}");
                }
            }
        }

        let human_count = participant_records.iter().filter(|player| !player.is_bot).count();
        let rating_is_hvn = mode == "HumansVsNations";
        let mut record = MatchRecord {
            schema_version: 1,
            match_id: match_id.to_string(),
            season_id: crate::profile::CURRENT_SEASON_ID,
            queue,
            mode,
            map_name,
            started_at: completed_at,
            completed_at,
            duration_seconds: 0,
            winner_account_id: winner,
            winning_team,
            verified: reports_consistent,
            rating_eligible: reports_consistent
                && if rating_is_hvn {
                    human_count >= 1
                } else {
                    human_count >= 2
                },
            participants: participant_records,
        };
        self.apply_ratings(&mut record)?;
        self.save_match_record(&record)?;

        let mut stats_keys: Vec<String> = Vec::new();
        for account_id in &players {
            stats_keys.push(format!("sow:match:{match_id}:stats:{account_id}"));
        }

        let _: () = redis::pipe()
            .set(&finalized_key, "1")
            .del(&players_key)
            .del(&exits_key)
            .query_async::<()>(&mut con)
            .await?;

        for key in stats_keys {
            let _: () = con.del(&key).await?;
        }
        for account_id in &players {
            let report_key = format!("sow:match:{match_id}:report:{account_id}");
            let _: () = con.del(&report_key).await?;
        }

        info!("Finalized match {match_id} with {} players", players.len());
        Ok(())
    }

    pub async fn submit_crazygames_score(&self, account: &PlayerAccount) {
        let Some(api_key) = &self.crazygames_api_key else {
            return;
        };

        if let Some(cg_identity) = account
            .linked_identities
            .iter()
            .find(|li| li.provider == "crazygames")
        {
            let score = account.profile.xp;
            let user_id = &cg_identity.external_id;
            info!("Submitting CrazyGames leaderboard score for user {user_id}: {score}");
            if let Err(e) = crate::crazygames::submit_score(api_key, user_id, score).await {
                error!("Failed to submit CrazyGames leaderboard score: {e}");
            }
        }
    }
}

fn is_valid_account_id(value: &str) -> bool {
    value.len() == ACCOUNT_ID_HEX_LEN && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::{
        DISPLAY_NAME_MAX_CHARS, generated_display_name, is_valid_account_id,
        normalize_display_name,
    };

    #[test]
    fn validates_canonical_account_ids() {
        assert!(is_valid_account_id("0123456789abcdef0123456789abcdef"));
        // The removed guest_<hex> format is intentionally rejected; only the
        // 32-hex canonical account ID is accepted by the live API.
        assert!(!is_valid_account_id(
            "guest_0123456789abcdef0123456789abcdef"
        ));
        assert!(!is_valid_account_id("not-an-account"));
    }

    #[test]
    fn normalizes_display_names_without_using_them_as_ids() {
        assert_eq!(normalize_display_name("  Alice  ").unwrap(), "Alice");
        assert_eq!(
            normalize_display_name(&"x".repeat(DISPLAY_NAME_MAX_CHARS + 4))
                .unwrap()
                .chars()
                .count(),
            DISPLAY_NAME_MAX_CHARS
        );
        assert!(normalize_display_name("\n\t").is_err());
        assert!(generated_display_name().starts_with("ANON"));
    }

    #[test]
    fn missing_display_name_is_detectable_for_one_time_migration() {
        let json = r#"{
            "id":"0123456789abcdef0123456789abcdef",
            "profile":{"xp":0,"level":1,"wins":0,"matches_played":0,
              "players_defeated":0,"empires_defeated":0,"tribes_defeated":0,
              "preferred_leader":null},
            "linked_identities":[],"created_at":0,"updated_at":0
        }"#;
        let account: super::PlayerAccount = serde_json::from_str(json).unwrap();
        assert!(account.display_name.is_empty());
        assert!(account.profile.leader_xp.is_empty());
        assert_eq!(account.profile.laurels, 0);
        assert!(!account.profile.intro_completed);
    }

    #[test]
    fn public_account_projection_does_not_serialize_secret_hash() {
        let json = r#"{
            "id":"0123456789abcdef0123456789abcdef",
            "profile":{"xp":0,"level":1,"wins":0,"matches_played":0,
              "players_defeated":0,"empires_defeated":0,"tribes_defeated":0,
              "preferred_leader":null},
            "linked_identities":[],"auth_secret_hash":"private","created_at":0,"updated_at":0
        }"#;
        let account: super::PlayerAccount = serde_json::from_str(json).unwrap();
        let public = serde_json::to_value(account.without_auth_secret()).unwrap();
        assert!(public.get("auth_secret_hash").is_none());
    }
}
