use crate::metadata_db::PLAYERS_TABLE;
use log::{error, info};
use redis::{AsyncCommands, Client};
use serde::{Deserialize, Serialize};

const ANALYTICS_UNIQUE: &str = "sow:analytics:unique_users";
const ANALYTICS_ACTIVE_PREFIX: &str = "sow:analytics:active:";
const ANALYTICS_DAU_TTL_SECS: i64 = 35 * 24 * 3600;

/// SET index of all bot account_ids — populated by `seed_bot_pool`, used for
/// pool introspection and analytics. Account records carry a canonical
/// display_name field; the bot allocator may choose its presentation name.
const BOT_POOL_KEY: &str = "sow:bot:pool";

const XP_WIN: u32 = 100;
const XP_MATCH: u32 = 20;

const XP_PER_PLAYER: u32 = 15;
const XP_PER_EMPIRE: u32 = 8;
const XP_PER_TRIBE: u32 = 2;
const XP_PER_ASSIST: u32 = 5;
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
            if let Err(error) = write_txn.commit() {
                error!("Failed to commit account {} to REDB: {error}", account.id);
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
        Ok(())
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
        // Provider identities are human accounts unless explicitly seeded as bots.
        let kind = if identity.provider == "bot" {
            AccountKind::Bot
        } else {
            AccountKind::Human
        };

        let new_account = PlayerAccount {
            id: random_id.clone(),
            display_name: generated_display_name(),
            profile: PlayerProfile::default(),
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

    /// Load or create an anonymous account using the canonical account ID.
    /// New anonymous accounts do not create a provider identity index.
    pub async fn get_or_create_anonymous(
        &self,
        account_id: Option<&str>,
        requested_display_name: Option<&str>,
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
                display_name: display_name.clone(),
                profile: PlayerProfile::default(),
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
                display_name: generated_display_name(),
                profile: PlayerProfile::default(),
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
                account.profile.record_match_with_kda(
                    won,
                    kda.defeats,
                    kda.kills,
                    kda.deaths,
                    kda.assists,
                );
                if let Some(leader) = preferred_leader.clone() {
                    account.profile.preferred_leader = Some(leader);
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
    }
}
