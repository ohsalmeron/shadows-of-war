use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use sow_core::protocol::AuthProof;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const PLAYGAMES_HANDOFF_TTL: Duration = Duration::from_secs(60);
const PLAYGAMES_SESSION_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const PLAYGAMES_RENDEZVOUS_TTL: Duration = Duration::from_secs(10);
const MAX_PLAYGAMES_RENDEZVOUS: usize = 256;

#[derive(Clone)]
pub(crate) struct IdentityState {
    inner: Arc<IdentityInner>,
}

struct IdentityInner {
    db_url: String,
    db_secret: String,
    handoffs: Mutex<HashMap<String, PlayGamesHandoff>>,
    rendezvous: Mutex<HashMap<String, PlayGamesRendezvous>>,
    sessions: Mutex<HashMap<String, PlayGamesSession>>,
    access_tokens: Mutex<HashMap<String, PlayGamesAccessToken>>,
}

struct PlayGamesHandoff {
    expires_at: Instant,
    account_id: String,
    external_id: String,
    environment: String,
    display_name: String,
    avatar_url: Option<String>,
}

struct PlayGamesRendezvous {
    expires_at: Instant,
    handoff_token: Option<String>,
}

struct PlayGamesSession {
    expires_at: Instant,
    account_id: String,
    external_id: String,
    environment: String,
    display_name: String,
    avatar_url: Option<String>,
}

struct PlayGamesAccessToken {
    access_token: String,
    expires_at: Instant,
}

#[derive(Clone, Debug)]
pub(crate) struct VerifiedIdentity {
    pub account_id: String,
    pub leader: sow_core::player::Leader,
}

#[derive(Deserialize)]
pub(crate) struct PlayGamesExchangeRequest {
    pub server_auth_code: String,
    pub package_name: String,
    #[serde(default)]
    pub rendezvous_id: Option<String>,
}

#[derive(Serialize)]
struct PlayGamesExchangeResponse {
    handoff_token: String,
}

#[derive(Deserialize)]
pub(crate) struct PlayGamesConsumeRequest {
    pub handoff_token: String,
}

#[derive(Deserialize)]
pub(crate) struct ProfileQuery {
    provider: String,
    external_id: String,
}

#[derive(Deserialize)]
pub(crate) struct PlayGamesPollQuery {
    pub rendezvous_id: String,
}

#[derive(Serialize)]
pub(crate) struct PlayGamesIdentityResponse {
    provider: &'static str,
    external_id: String,
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
    name_locked: bool,
    token: String,
}

pub(crate) enum PlayGamesRendezvousPoll {
    Pending,
    Expired,
    Ready(PlayGamesIdentityResponse),
}

#[derive(Deserialize)]
struct PlayGamesOAuthTokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(Deserialize)]
struct PlayGamesPlayerResponse {
    #[serde(rename = "playerId")]
    player_id: String,
    #[serde(rename = "displayName", default)]
    display_name: String,
    #[serde(rename = "avatarImageUrl", default)]
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct CrazyTokenClaims {
    #[serde(rename = "userId")]
    user_id: String,
}

#[derive(Deserialize)]
struct CrazyPublicKeyResponse {
    #[serde(rename = "publicKey")]
    public_key: String,
}

#[derive(Deserialize)]
struct DbIdentityResponse {
    account_id: String,
    #[serde(default)]
    leader: Option<String>,
    account: serde_json::Value,
}

#[derive(Deserialize)]
struct DbVerifyResponse {
    account_id: String,
    leader: Option<String>,
}

#[derive(Serialize)]
struct VerifiedIdentityRequest<'a> {
    provider: &'a str,
    environment: &'a str,
    external_subject: &'a str,
    display_name: Option<&'a str>,
    avatar_url: Option<&'a str>,
    requested_leader: Option<&'a str>,
}

impl IdentityState {
    pub(crate) fn from_env(db_url: String, db_secret: String) -> Self {
        Self {
            inner: Arc::new(IdentityInner {
                db_url,
                db_secret,
                handoffs: Mutex::new(HashMap::new()),
                rendezvous: Mutex::new(HashMap::new()),
                sessions: Mutex::new(HashMap::new()),
                access_tokens: Mutex::new(HashMap::new()),
            }),
        }
    }

    async fn db_resolve(
        &self,
        provider: &str,
        environment: &str,
        external_subject: &str,
        display_name: Option<&str>,
        avatar_url: Option<&str>,
        requested_leader: Option<&str>,
    ) -> Result<DbIdentityResponse, String> {
        if provider.trim().is_empty()
            || environment.trim().is_empty()
            || external_subject.trim().is_empty()
        {
            return Err("verified identity fields are incomplete".to_string());
        }
        let url = format!(
            "{}/internal/identity/resolve",
            self.inner.db_url.trim_end_matches('/')
        );
        let response = reqwest::Client::new()
            .post(url)
            .header("Authorization", format!("Bearer {}", self.inner.db_secret))
            .json(&VerifiedIdentityRequest {
                provider,
                environment,
                external_subject,
                display_name,
                avatar_url,
                requested_leader,
            })
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|error| format!("identity database request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "identity database returned HTTP {}",
                response.status()
            ));
        }
        response
            .json::<DbIdentityResponse>()
            .await
            .map_err(|error| format!("identity database response unreadable: {error}"))
    }

    async fn verify_external(
        &self,
        provider: &str,
        external_id: Option<&str>,
        token: &str,
    ) -> Result<(String, String, Option<String>, Option<String>), String> {
        match provider {
            "playgames" => {
                let identity = self.verify_playgames_session(external_id, token)?;
                Ok((
                    identity.external_id,
                    identity.environment,
                    Some(identity.display_name),
                    identity.avatar_url,
                ))
            }
            "wou" | "wou_id" | "world_of_unreal" => {
                let wou_url = std::env::var("WOU_ID_URL")
                    .unwrap_or_else(|_| "https://id.worldofunreal.com".to_string());
                let mut builder = reqwest::Client::builder();
                if let Ok(raw_ip) = std::env::var("WOU_ID_RESOLVE_IP") {
                    let host = reqwest::Url::parse(&wou_url)
                        .ok()
                        .and_then(|url| url.host_str().map(str::to_owned))
                        .ok_or_else(|| "WOU_ID_URL has no valid hostname".to_string())?;
                    let port = reqwest::Url::parse(&wou_url)
                        .ok()
                        .and_then(|url| url.port_or_known_default())
                        .unwrap_or(443);
                    let ip = raw_ip
                        .parse()
                        .map_err(|_| "WOU_ID_RESOLVE_IP is not a valid IP address".to_string())?;
                    builder = builder.resolve(&host, std::net::SocketAddr::new(ip, port));
                }
                let response = builder
                    .build()
                    .map_err(|error| format!("WOU-ID client build failed: {error}"))?
                    .get(format!(
                        "{}/api/v1/inventory/me",
                        wou_url.trim_end_matches('/')
                    ))
                    .header("Authorization", format!("Bearer {token}"))
                    .timeout(Duration::from_secs(3))
                    .send()
                    .await
                    .map_err(|error| format!("WOU-ID service unreachable: {error}"))?;
                if !response.status().is_success() {
                    return Err(format!("WOU-ID token rejected: HTTP {}", response.status()));
                }
                let body: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|error| format!("invalid WOU-ID response: {error}"))?;
                let subject = body
                    .get("account_id")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "WOU-ID response missing account_id".to_string())?;
                Ok((subject.to_string(), "production".to_string(), None, None))
            }
            "crazygames" => {
                let response = reqwest::get("https://sdk.crazygames.com/publicKey.json")
                    .await
                    .map_err(|error| format!("CrazyGames public key unavailable: {error}"))?;
                let key_body: CrazyPublicKeyResponse = response
                    .json()
                    .await
                    .map_err(|error| format!("invalid CrazyGames public key response: {error}"))?;
                let key = DecodingKey::from_rsa_pem(key_body.public_key.as_bytes())
                    .map_err(|error| format!("invalid CrazyGames public key: {error}"))?;
                let mut validation = Validation::new(Algorithm::RS256);
                validation.validate_exp = true;
                let claims = decode::<CrazyTokenClaims>(token, &key, &validation)
                    .map_err(|error| format!("CrazyGames token rejected: {error}"))?
                    .claims;
                if claims.user_id.trim().is_empty() {
                    return Err("CrazyGames token missing userId".to_string());
                }
                if external_id.is_some_and(|value| !value.is_empty() && value != claims.user_id) {
                    return Err("CrazyGames player mismatch".to_string());
                }
                Ok((claims.user_id, "production".to_string(), None, None))
            }
            _ => Err(format!("unsupported provider: {provider}")),
        }
    }

    pub(crate) async fn verify_auth_proof(
        &self,
        auth: &AuthProof,
        requested_leader: sow_core::player::Leader,
    ) -> Result<VerifiedIdentity, String> {
        let requested = sow_core::commerce::leader_wire_id(requested_leader).to_string();
        if auth.provider == "anonymous" {
            let url = format!(
                "{}/internal/verify",
                self.inner.db_url.trim_end_matches('/')
            );
            let response = reqwest::Client::new()
                .post(url)
                .header("Authorization", format!("Bearer {}", self.inner.db_secret))
                .json(&serde_json::json!({
                    "provider": "anonymous",
                    "account_id": auth.account_id,
                    "token": auth.token,
                    "requested_leader": requested,
                }))
                .timeout(Duration::from_secs(5))
                .send()
                .await
                .map_err(|error| format!("anonymous verification failed: {error}"))?;
            if !response.status().is_success() {
                return Err(format!(
                    "anonymous verification returned HTTP {}",
                    response.status()
                ));
            }
            let body: DbVerifyResponse = response
                .json()
                .await
                .map_err(|error| format!("anonymous verification response unreadable: {error}"))?;
            let leader = body
                .leader
                .as_deref()
                .and_then(sow_core::commerce::leader_from_id)
                .ok_or_else(|| "anonymous verification missing authorized leader".to_string())?;
            return Ok(VerifiedIdentity {
                account_id: body.account_id,
                leader,
            });
        }
        let (subject, environment, display_name, avatar_url) = self
            .verify_external(
                auth.provider.trim(),
                auth.account_id.as_deref(),
                &auth.token,
            )
            .await?;
        let body = self
            .db_resolve(
                auth.provider.trim(),
                &environment,
                &subject,
                display_name.as_deref(),
                avatar_url.as_deref(),
                Some(&requested),
            )
            .await?;
        let leader = body
            .leader
            .as_deref()
            .and_then(sow_core::commerce::leader_from_id)
            .ok_or_else(|| "identity verification missing authorized leader".to_string())?;
        Ok(VerifiedIdentity {
            account_id: body.account_id,
            leader,
        })
    }

    pub(crate) async fn platform_profile(
        &self,
        provider: &str,
        external_id: &str,
        token: &str,
    ) -> Result<serde_json::Value, String> {
        let (subject, environment, display_name, avatar_url) = self
            .verify_external(provider, Some(external_id), token)
            .await?;
        Ok(self
            .db_resolve(
                provider,
                &environment,
                &subject,
                display_name.as_deref(),
                avatar_url.as_deref(),
                None,
            )
            .await?
            .account)
    }

    pub(crate) async fn exchange_playgames(
        &self,
        payload: PlayGamesExchangeRequest,
    ) -> Result<String, String> {
        if !matches!(
            payload.package_name.as_str(),
            "com.shadowsofwar" | "com.shadowsofwar.debug"
        ) {
            return Err("unsupported Android package".to_string());
        }
        if payload.server_auth_code.trim().is_empty() || payload.server_auth_code.len() > 4096 {
            return Err("invalid Play Games server auth code".to_string());
        }
        if let Some(rendezvous_id) = payload.rendezvous_id.as_deref()
            && !valid_rendezvous_id(rendezvous_id)
        {
            return Err("invalid Play Games rendezvous ID".to_string());
        }
        let client_id = std::env::var("SOW_PLAY_GAMES_WEB_CLIENT_ID")
            .map_err(|_| "Play Games server access is not configured".to_string())?;
        let client_secret = std::env::var("SOW_PLAY_GAMES_WEB_CLIENT_SECRET")
            .map_err(|_| "Play Games server access is not configured".to_string())?;
        let client = reqwest::Client::new();
        let token_response = client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("code", payload.server_auth_code.as_str()),
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("grant_type", "authorization_code"),
            ])
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|error| format!("Play Games token exchange failed: {error}"))?;
        if !token_response.status().is_success() {
            return Err("Play Games authentication was rejected".to_string());
        }
        let token_response: PlayGamesOAuthTokenResponse = token_response
            .json()
            .await
            .map_err(|error| format!("Play Games token response unreadable: {error}"))?;
        let player_response = client
            .get("https://games.googleapis.com/games/v1/players/me")
            .bearer_auth(&token_response.access_token)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|error| format!("Play Games player verification failed: {error}"))?;
        if !player_response.status().is_success() {
            return Err("Play Games player verification was rejected".to_string());
        }
        let player: PlayGamesPlayerResponse = player_response
            .json()
            .await
            .map_err(|error| format!("Play Games player response unreadable: {error}"))?;
        if player.player_id.trim().is_empty() {
            return Err("Play Games response did not include a player ID".to_string());
        }
        let environment = if payload.package_name.ends_with(".debug") {
            "debug"
        } else {
            "production"
        };
        let account = self
            .db_resolve(
                "playgames",
                environment,
                &player.player_id,
                Some(&player.display_name),
                player.avatar_url.as_deref(),
                None,
            )
            .await?;
        let ttl = token_response
            .expires_in
            .unwrap_or(3600)
            .saturating_sub(30)
            .max(60);
        self.inner
            .access_tokens
            .lock()
            .map_err(|_| "Play Games access-token store is poisoned".to_string())?
            .insert(
                account.account_id.clone(),
                PlayGamesAccessToken {
                    access_token: token_response.access_token,
                    expires_at: Instant::now() + Duration::from_secs(ttl),
                },
            );
        let handoff_token = random_token();
        self.inner
            .handoffs
            .lock()
            .map_err(|_| "Play Games handoff store is poisoned".to_string())?
            .retain(|_, value| value.expires_at > Instant::now());
        self.inner
            .handoffs
            .lock()
            .map_err(|_| "Play Games handoff store is poisoned".to_string())?
            .insert(
                handoff_token.clone(),
                PlayGamesHandoff {
                    expires_at: Instant::now() + PLAYGAMES_HANDOFF_TTL,
                    account_id: account.account_id,
                    external_id: player.player_id,
                    environment: environment.to_string(),
                    display_name: player.display_name,
                    avatar_url: player.avatar_url,
                },
            );
        if let Some(rendezvous_id) = payload.rendezvous_id.as_deref() {
            let mut rendezvous = self
                .inner
                .rendezvous
                .lock()
                .map_err(|_| "Play Games rendezvous store is poisoned".to_string())?;
            let now = Instant::now();
            rendezvous.retain(|_, value| value.expires_at > now);
            if rendezvous.len() < MAX_PLAYGAMES_RENDEZVOUS || rendezvous.contains_key(rendezvous_id)
            {
                rendezvous
                    .entry(rendezvous_id.to_string())
                    .or_insert(PlayGamesRendezvous {
                        expires_at: now + PLAYGAMES_RENDEZVOUS_TTL,
                        handoff_token: None,
                    })
                    .handoff_token = Some(handoff_token.clone());
            }
        }
        Ok(handoff_token)
    }

    pub(crate) fn poll_playgames(
        &self,
        rendezvous_id: &str,
    ) -> Result<PlayGamesRendezvousPoll, String> {
        if !valid_rendezvous_id(rendezvous_id) {
            return Err("invalid Play Games rendezvous ID".to_string());
        }
        let handoff_token = {
            let mut rendezvous = self
                .inner
                .rendezvous
                .lock()
                .map_err(|_| "Play Games rendezvous store is poisoned".to_string())?;
            let now = Instant::now();
            if rendezvous
                .get(rendezvous_id)
                .is_some_and(|value| value.expires_at <= now)
            {
                rendezvous.remove(rendezvous_id);
                return Ok(PlayGamesRendezvousPoll::Expired);
            }
            rendezvous.retain(|_, value| value.expires_at > now);
            match rendezvous.get(rendezvous_id) {
                Some(value) => value.handoff_token.clone(),
                None => {
                    if rendezvous.len() >= MAX_PLAYGAMES_RENDEZVOUS {
                        return Err("Play Games rendezvous capacity exhausted".to_string());
                    }
                    rendezvous.insert(
                        rendezvous_id.to_string(),
                        PlayGamesRendezvous {
                            expires_at: now + PLAYGAMES_RENDEZVOUS_TTL,
                            handoff_token: None,
                        },
                    );
                    return Ok(PlayGamesRendezvousPoll::Pending);
                }
            }
        };

        let Some(handoff_token) = handoff_token else {
            return Ok(PlayGamesRendezvousPoll::Pending);
        };
        self.inner
            .rendezvous
            .lock()
            .map_err(|_| "Play Games rendezvous store is poisoned".to_string())?
            .remove(rendezvous_id);
        match self.consume_playgames(PlayGamesConsumeRequest { handoff_token }) {
            Ok(identity) => Ok(PlayGamesRendezvousPoll::Ready(identity)),
            Err(_) => Ok(PlayGamesRendezvousPoll::Expired),
        }
    }

    pub(crate) fn consume_playgames(
        &self,
        payload: PlayGamesConsumeRequest,
    ) -> Result<PlayGamesIdentityResponse, String> {
        let handoff = self
            .inner
            .handoffs
            .lock()
            .map_err(|_| "Play Games handoff store is poisoned".to_string())?
            .remove(payload.handoff_token.trim());
        let Some(handoff) = handoff.filter(|value| value.expires_at > Instant::now()) else {
            return Err("Play Games handoff expired or already used".to_string());
        };
        let token = random_token();
        self.inner
            .sessions
            .lock()
            .map_err(|_| "Play Games session store is poisoned".to_string())?
            .retain(|_, value| value.expires_at > Instant::now());
        self.inner
            .sessions
            .lock()
            .map_err(|_| "Play Games session store is poisoned".to_string())?
            .insert(
                token.clone(),
                PlayGamesSession {
                    expires_at: Instant::now() + PLAYGAMES_SESSION_TTL,
                    account_id: handoff.account_id,
                    external_id: handoff.external_id.clone(),
                    environment: handoff.environment,
                    display_name: handoff.display_name.clone(),
                    avatar_url: handoff.avatar_url.clone(),
                },
            );
        Ok(PlayGamesIdentityResponse {
            provider: "playgames",
            external_id: handoff.external_id,
            display_name: handoff.display_name,
            avatar_url: handoff.avatar_url,
            name_locked: true,
            token,
        })
    }

    fn verify_playgames_session(
        &self,
        external_id: Option<&str>,
        token: &str,
    ) -> Result<PlayGamesSession, String> {
        let mut sessions = self
            .inner
            .sessions
            .lock()
            .map_err(|_| "Play Games session store is poisoned".to_string())?;
        sessions.retain(|_, value| value.expires_at > Instant::now());
        let session = sessions
            .get(token.trim())
            .ok_or_else(|| "Play Games session expired or invalid".to_string())?;
        if external_id.is_some_and(|value| value != session.external_id) {
            return Err("Play Games player mismatch".to_string());
        }
        Ok(PlayGamesSession {
            expires_at: session.expires_at,
            account_id: session.account_id.clone(),
            external_id: session.external_id.clone(),
            environment: session.environment.clone(),
            display_name: session.display_name.clone(),
            avatar_url: session.avatar_url.clone(),
        })
    }
}

fn random_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    hex::encode(bytes)
}

fn valid_rendezvous_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) async fn handle_playgames_exchange(
    State(state): State<crate::AppState>,
    Json(payload): Json<PlayGamesExchangeRequest>,
) -> impl IntoResponse {
    match state.identity.exchange_playgames(payload).await {
        Ok(handoff_token) => (
            StatusCode::OK,
            Json(PlayGamesExchangeResponse { handoff_token }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn handle_playgames_consume(
    State(state): State<crate::AppState>,
    Json(payload): Json<PlayGamesConsumeRequest>,
) -> impl IntoResponse {
    match state.identity.consume_playgames(payload) {
        Ok(identity) => (StatusCode::OK, Json(identity)).into_response(),
        Err(error) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn handle_playgames_poll(
    State(state): State<crate::AppState>,
    Query(query): Query<PlayGamesPollQuery>,
) -> impl IntoResponse {
    match state.identity.poll_playgames(query.rendezvous_id.trim()) {
        Ok(PlayGamesRendezvousPoll::Pending) => StatusCode::NO_CONTENT.into_response(),
        Ok(PlayGamesRendezvousPoll::Expired) => StatusCode::GONE.into_response(),
        Ok(PlayGamesRendezvousPoll::Ready(identity)) => {
            (StatusCode::OK, Json(identity)).into_response()
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn handle_profile(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Query(query): Query<ProfileQuery>,
) -> impl IntoResponse {
    let token = headers
        .get("x-platform-auth")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default();
    match state
        .identity
        .platform_profile(query.provider.trim(), query.external_id.trim(), token)
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account)).into_response(),
        Err(error) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> IdentityState {
        IdentityState::from_env("http://127.0.0.1:25585".to_string(), "secret".to_string())
    }

    #[test]
    fn rendezvous_starts_pending() {
        let result = state().poll_playgames(&"a".repeat(32)).unwrap();
        assert!(matches!(result, PlayGamesRendezvousPoll::Pending));
    }

    #[test]
    fn rendezvous_expires_without_consuming_a_handoff() {
        let state = state();
        let rendezvous_id = "b".repeat(32);
        state.inner.rendezvous.lock().unwrap().insert(
            rendezvous_id.clone(),
            PlayGamesRendezvous {
                expires_at: Instant::now() - Duration::from_secs(1),
                handoff_token: None,
            },
        );
        let result = state.poll_playgames(&rendezvous_id).unwrap();
        assert!(matches!(result, PlayGamesRendezvousPoll::Expired));
    }

    #[test]
    fn rendezvous_consumes_a_ready_handoff_once() {
        let state = state();
        let rendezvous_id = "c".repeat(32);
        let handoff_token = "d".repeat(64);
        state.inner.handoffs.lock().unwrap().insert(
            handoff_token.clone(),
            PlayGamesHandoff {
                expires_at: Instant::now() + PLAYGAMES_HANDOFF_TTL,
                account_id: "account".to_string(),
                external_id: "player".to_string(),
                environment: "debug".to_string(),
                display_name: "Player".to_string(),
                avatar_url: None,
            },
        );
        state.inner.rendezvous.lock().unwrap().insert(
            rendezvous_id.clone(),
            PlayGamesRendezvous {
                expires_at: Instant::now() + PLAYGAMES_RENDEZVOUS_TTL,
                handoff_token: Some(handoff_token),
            },
        );

        let result = state.poll_playgames(&rendezvous_id).unwrap();
        let PlayGamesRendezvousPoll::Ready(identity) = result else {
            panic!("rendezvous was not ready");
        };
        assert_eq!(identity.external_id, "player");
        assert!(matches!(
            state.poll_playgames(&rendezvous_id).unwrap(),
            PlayGamesRendezvousPoll::Pending
        ));
    }
}
