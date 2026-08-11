use crate::metadata_db::PLAYERS_TABLE;
use log::{error, info, warn};
use redis::{AsyncCommands, Client};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const ANALYTICS_UNIQUE: &str = "sow:analytics:unique_users";
const ANALYTICS_ACTIVE_PREFIX: &str = "sow:analytics:active:";
const ANALYTICS_DAU_TTL_SECS: i64 = 35 * 24 * 3600;

/// SET index of all bot account_ids — populated by `seed_bot_pool`, used for
/// pool introspection and analytics. The (account_id → display_name) mapping
/// is held by the caller (sow-server), not stored here.
const BOT_POOL_KEY: &str = "sow:bot:pool";

const XP_WIN: u32 = 100;
const XP_MATCH: u32 = 20;

const XP_PER_PLAYER: u32 = 15;
const XP_PER_EMPIRE: u32 = 8;
const XP_PER_TRIBE: u32 = 2;
const XP_PER_ASSIST: u32 = 5;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct SessionDefeats {
    pub players: u32,
    pub empires: u32,
    pub tribes: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct MatchOutcomeKda {
    pub defeats: SessionDefeats,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
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
        }
    }
}

impl PlayerProfile {
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerAccount {
    pub id: String, // Stable canonical internal account ID
    pub profile: PlayerProfile,
    pub linked_identities: Vec<LinkedIdentity>,
    /// Account kind. Defaults to Human for legacy accounts (serde default).
    /// Bots are accounts with `kind = Bot` — they have profiles, accumulate
    /// stats, and serve as the persistent identity pool for internal fillers.
    #[serde(default)]
    pub kind: AccountKind,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Distinguishes real players from persistent bot accounts. Bots are
/// full-fledged accounts (they have stats, profiles) — they just aren't
/// driven by a human. Used for stat filtering, leaderboards, display.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountKind {
    Human,
    Bot,
}

impl Default for AccountKind {
    fn default() -> Self {
        Self::Human
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash, PartialEq, Eq)]
pub struct LinkedIdentity {
    pub provider: String, // "crazygames", "poki", "steam", "playgames", "gamecenter", "local", "bot"
    pub external_id: String, // Stable unique ID from the platform
}

#[derive(Serialize, Clone, Debug)]
pub struct AccountSummary {
    pub id: String,
    pub level: u32,
    pub xp: u32,
    pub wins: u32,
    pub matches_played: u32,
}

impl From<&PlayerAccount> for AccountSummary {
    fn from(a: &PlayerAccount) -> Self {
        Self {
            id: a.id.clone(),
            level: a.profile.level,
            xp: a.profile.xp,
            wins: a.profile.wins,
            matches_played: a.profile.matches_played,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub enum LinkOutcome {
    Linked(PlayerAccount),
    Conflict {
        existing_account_id: String,
        existing: AccountSummary,
        current: AccountSummary,
    },
}

#[derive(Clone)]
pub struct PlayerDb {
    client: Client,
    pub crazygames_api_key: Option<String>,
    pub metadata_db: Option<std::sync::Arc<redb::Database>>,
}

fn utc_date_string() -> String {
    let dt = crate::time_util::now_utc();
    format!("{:04}-{:02}-{:02}", dt.year, dt.month, dt.day)
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
        if let Some(ref db) = self.metadata_db {
            if let Ok(write_txn) = db.begin_write() {
                if let Ok(mut table) = write_txn.open_table(PLAYERS_TABLE) {
                    if let Ok(json) = serde_json::to_string(account) {
                        let _ = table.insert(account.id.as_str(), json.as_bytes());
                    }
                }
                let _ = write_txn.commit();
            }
        }
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

    async fn save_account(
        con: &mut redis::aio::MultiplexedConnection,
        account: &PlayerAccount,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let acc_key = Self::account_key(&account.id);
        let json = serde_json::to_string(account)?;
        con.set::<_, _, ()>(&acc_key, json).await?;
        Ok(())
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
        Ok(())
    }

    /// Get or create player account by platform identity
    pub async fn get_or_create(
        &self,
        provider: String,
        external_id: String,
        _fallback_name: String,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let id_key = Self::identity_key(&provider, &external_id);

        // 1. Try to find existing account ID mapped to this identity
        if let Some(account_id) = con.get::<_, Option<String>>(&id_key).await?
            && let Ok(account) = Self::load_account(&mut con, &account_id).await
        {
            let _: () = Self::record_analytics(&mut con, &account.id, false).await?;
            return Ok(account);
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
        // other providers — including legacy accounts without a `kind` field
        // in storage — default to Human via serde.
        let kind = if identity.provider == "bot" {
            AccountKind::Bot
        } else {
            AccountKind::Human
        };

        let new_account = PlayerAccount {
            id: random_id.clone(),
            profile: PlayerProfile::default(),
            linked_identities: vec![identity.clone()],
            kind,
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

    /// Seed the persistent bot-account pool. For each external_id, performs
    /// a get-or-create with `provider = "bot"` (so created accounts carry
    /// `kind = Bot`). Idempotent — re-running with the same external_ids
    /// returns the existing account_ids without duplication. Also maintains
    /// the `sow:bot:pool` SET as an index of all bot account_ids for
    /// analytics / debugging.
    ///
    /// The display name is NOT stored here — it is derived deterministically
    /// by the caller (sow-server) from the external_id index, so the caller
    /// keeps the (account_id, display_name) mapping in its local cache.
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
                profile: PlayerProfile::default(),
                linked_identities: vec![identity.clone()],
                kind: AccountKind::Bot,
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

        info!("[bot-pool] seed resolved {} bot accounts", account_ids.len());
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

        if let Some(acc_json) = con.get::<_, Option<String>>(&acc_key).await? {
            let mut account: PlayerAccount = serde_json::from_str(&acc_json)?;
            account.profile = profile;
            account.updated_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let updated_json = serde_json::to_string(&account)?;
            con.set::<_, _, ()>(&acc_key, updated_json).await?;

            self.save_player_account_to_redb(&account);
            Ok(account)
        } else {
            Err("Account not found".into())
        }
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

        if let Some(acc_json) = con.get::<_, Option<String>>(&acc_key).await? {
            let mut account: PlayerAccount = serde_json::from_str(&acc_json)?;
            account.profile.record_match_with_kda(
                won,
                kda.defeats,
                kda.kills,
                kda.deaths,
                kda.assists,
            );
            if let Some(leader) = preferred_leader {
                account.profile.preferred_leader = Some(leader);
            }
            account.updated_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let updated_json = serde_json::to_string(&account)?;
            con.set::<_, _, ()>(&acc_key, updated_json).await?;

            self.save_player_account_to_redb(&account);
            Ok(account)
        } else {
            Err("Account not found".into())
        }
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

    /// Finalize match from relay-logged exit order in Valkey.
    pub async fn finalize_match(
        &self,
        match_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let finalized_key = format!("sow:match:{match_id}:finalized");
        if con.exists(&finalized_key).await? {
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
        let winner = players
            .iter()
            .find(|p| !exits.contains(p))
            .cloned()
            .or_else(|| exits.last().cloned());

        let opponent_count = players.len().saturating_sub(1) as u32;
        for account_id in &players {
            let won = winner.as_ref() == Some(account_id);
            let stats_key = format!("sow:match:{match_id}:stats:{account_id}");
            let stats: Option<std::collections::HashMap<String, String>> =
                con.hgetall(&stats_key).await?;

            let (defeats, kills, deaths, assists) = if let Some(map) = stats {
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
                )
            };

            match self
                .record_match_outcome_with_kda(
                    account_id,
                    won,
                    MatchOutcomeKda {
                        defeats,
                        kills,
                        deaths,
                        assists,
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

        let mut stats_keys: Vec<String> = Vec::new();
        for account_id in &players {
            stats_keys.push(format!("sow:match:{match_id}:stats:{account_id}"));
        }

        let _: () = redis::pipe()
            .set(&finalized_key, "1")
            .expire(&finalized_key, 3600)
            .del(&players_key)
            .del(&exits_key)
            .query_async::<()>(&mut con)
            .await?;

        for key in stats_keys {
            let _: () = con.del(&key).await?;
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

    /// Link a new identity to an existing account (internal; errors on conflict).
    pub async fn link_identity(
        &self,
        account_id: &str,
        provider: String,
        external_id: String,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let account =
            Self::link_identity_inner(&mut con, account_id, provider, external_id).await?;
        self.save_player_account_to_redb(&account);
        Ok(account)
    }

    /// Client-facing link attempt; returns conflict metadata when identity maps elsewhere.
    pub async fn try_link_identity(
        &self,
        account_id: &str,
        provider: String,
        external_id: String,
    ) -> Result<LinkOutcome, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let id_key = Self::identity_key(&provider, &external_id);

        if let Some(linked_id) = con.get::<_, Option<String>>(&id_key).await? {
            if linked_id == account_id {
                let account = Self::load_account(&mut con, account_id).await?;
                return Ok(LinkOutcome::Linked(account));
            }
            let existing = Self::load_account(&mut con, &linked_id).await?;
            let current = Self::load_account(&mut con, account_id).await?;
            return Ok(LinkOutcome::Conflict {
                existing_account_id: linked_id,
                existing: AccountSummary::from(&existing),
                current: AccountSummary::from(&current),
            });
        }

        let account =
            Self::link_identity_inner(&mut con, account_id, provider, external_id).await?;
        self.save_player_account_to_redb(&account);
        Ok(LinkOutcome::Linked(account))
    }

    /// Resolve an identity link conflict by keeping one canonical account.
    pub async fn resolve_link_conflict(
        &self,
        current_account_id: &str,
        keep_account_id: &str,
        provider: String,
        external_id: String,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let mut con = self.get_connection().await?;
        let id_key = Self::identity_key(&provider, &external_id);
        let mapped: Option<String> = con.get(&id_key).await?;

        if keep_account_id != current_account_id && mapped.as_deref() != Some(keep_account_id) {
            return Err("keep_account_id must be current or the existing mapped account".into());
        }

        let account = if keep_account_id == current_account_id {
            if let Some(other_id) = mapped
                && other_id != current_account_id
            {
                Self::unlink_identity_from_account(&mut con, &other_id, &provider, &external_id)
                    .await?;
                Self::delete_account_if_orphan(&mut con, &other_id, &self.metadata_db).await?;
            }
            Self::link_identity_inner(&mut con, current_account_id, provider, external_id).await?
        } else {
            Self::transfer_local_identities(&mut con, current_account_id, keep_account_id).await?;
            Self::delete_account_if_orphan(&mut con, current_account_id, &self.metadata_db).await?;
            Self::load_account(&mut con, keep_account_id).await?
        };

        self.save_player_account_to_redb(&account);
        Ok(account)
    }

    async fn link_identity_inner(
        con: &mut redis::aio::MultiplexedConnection,
        account_id: &str,
        provider: String,
        external_id: String,
    ) -> Result<PlayerAccount, Box<dyn std::error::Error + Send + Sync>> {
        let id_key = Self::identity_key(&provider, &external_id);

        if let Some(linked_id) = con.get::<_, Option<String>>(&id_key).await?
            && linked_id != account_id
        {
            return Err("Identity already linked to another account".into());
        }

        let mut account = Self::load_account(con, account_id).await?;
        let identity = LinkedIdentity {
            provider,
            external_id,
        };

        if !account.linked_identities.contains(&identity) {
            account.linked_identities.push(identity);
            account.updated_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let acc_key = Self::account_key(account_id);
            let updated_json = serde_json::to_string(&account)?;

            redis::pipe()
                .set(&acc_key, updated_json)
                .set(&id_key, account_id)
                .query_async::<()>(con)
                .await?;
        }

        Ok(account)
    }

    async fn unlink_identity_from_account(
        con: &mut redis::aio::MultiplexedConnection,
        account_id: &str,
        provider: &str,
        external_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let id_key = Self::identity_key(provider, external_id);
        let mut account = Self::load_account(con, account_id).await?;
        account
            .linked_identities
            .retain(|i| !(i.provider == provider && i.external_id == external_id));
        account.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self::save_account(con, &account).await?;
        let _: () = con.del(&id_key).await?;
        Ok(())
    }

    async fn transfer_local_identities(
        con: &mut redis::aio::MultiplexedConnection,
        from_id: &str,
        to_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let from = Self::load_account(con, from_id).await?;
        let locals: Vec<LinkedIdentity> = from
            .linked_identities
            .iter()
            .filter(|i| i.provider == "local")
            .cloned()
            .collect();
        for identity in locals {
            Self::unlink_identity_from_account(
                con,
                from_id,
                &identity.provider,
                &identity.external_id,
            )
            .await?;
            let _ = Self::link_identity_inner(con, to_id, identity.provider, identity.external_id)
                .await;
        }
        Ok(())
    }

    async fn delete_account_if_orphan(
        con: &mut redis::aio::MultiplexedConnection,
        account_id: &str,
        metadata_db: &Option<std::sync::Arc<redb::Database>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let account = match Self::load_account(con, account_id).await {
            Ok(a) => a,
            Err(_) => return Ok(()),
        };
        if !account.linked_identities.is_empty() {
            return Ok(());
        }
        let acc_key = Self::account_key(account_id);
        let _: () = con.del(&acc_key).await?;
        warn!("Deleted orphan account {account_id}");

        if let Some(db) = metadata_db {
            if let Ok(write_txn) = db.begin_write() {
                if let Ok(mut table) = write_txn.open_table(PLAYERS_TABLE) {
                    let _ = table.remove(account_id);
                }
                let _ = write_txn.commit();
            }
        }
        Ok(())
    }
}
