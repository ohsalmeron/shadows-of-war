use sow_data::crazygames;
use sow_data::db::{PlayGamesMatchOutcome, PlayerDb, PlayerProfile};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use log::{error, info, warn};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tower_http::cors::{Any, CorsLayer};

const MAX_REPLAY_BYTES: usize = 16 * 1024 * 1024;
const MAX_REPLAY_REQUEST_BYTES: usize = 32 * 1024 * 1024;

struct AppState {
    db: PlayerDb,
    secret_token: String,
    revenuecat_webhook_secret: Option<String>,
    redb_path: String,
    events: std::sync::Mutex<sow_data::events::EventSink>,
    playgames_handoffs: std::sync::Mutex<HashMap<String, PlayGamesHandoff>>,
    playgames_sessions: std::sync::Mutex<HashMap<String, PlayGamesSession>>,
    playgames_access_tokens: std::sync::Mutex<HashMap<String, PlayGamesAccessToken>>,
}

const PLAYGAMES_HANDOFF_TTL: Duration = Duration::from_secs(60);
const PLAYGAMES_SESSION_TTL: Duration = Duration::from_secs(24 * 60 * 60);

struct PlayGamesHandoff {
    expires_at: Instant,
    account_id: String,
    external_id: String,
    display_name: String,
    avatar_url: Option<String>,
}

struct PlayGamesSession {
    expires_at: Instant,
    account_id: String,
    external_id: String,
}

struct PlayGamesAccessToken {
    access_token: String,
    expires_at: Instant,
}

struct VerifiedPlayGamesIdentity {
    account_id: String,
    external_id: String,
}

#[derive(Deserialize)]
struct ProfileQuery {
    provider: String,
    external_id: String,
}

#[derive(Deserialize)]
struct AnonymousProfileRequest {
    account_id: Option<String>,
    display_name: Option<String>,
    #[serde(default)]
    auth_secret: Option<String>,
}

#[derive(Deserialize)]
struct AnonymousDisplayNameRequest {
    account_id: String,
    display_name: String,
    auth_secret: String,
}

#[derive(Deserialize)]
struct TutorialCompleteRequest {
    account_id: String,
    auth_secret: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct MatchStartRequest {
    match_id: String,
    player_ids: Vec<String>,
}

#[derive(Deserialize)]
struct MatchFinalizeRequest {
    match_id: String,
    #[serde(default)]
    lobby_json: Option<String>,
    #[serde(default)]
    replay_data: Option<Vec<u8>>,
}

#[derive(Deserialize)]
struct DirectSaveRequest {
    account_id: String,
    profile: PlayerProfile,
}

#[derive(Deserialize)]
struct BotPoolSeedRequest {
    external_ids: Vec<String>,
}

#[derive(Deserialize, Default)]
struct PublicSearchQuery {
    q: Option<String>,
    cursor: Option<usize>,
    limit: Option<usize>,
}

#[derive(Deserialize, Default)]
struct PublicHistoryQuery {
    cursor: Option<usize>,
    limit: Option<usize>,
    queue: Option<String>,
    mode: Option<String>,
}

#[derive(Deserialize, Default)]
struct PublicLeaderboardQuery {
    queue: Option<String>,
    mode: Option<String>,
    cursor: Option<usize>,
    limit: Option<usize>,
}

/// POST /internal/verify — resolve an identity proof to a canonical account
/// id. Callers are internal services (sow-server, sow-backfill) authenticated
/// by the deployment bearer secret; the proof itself is what establishes the
/// player's identity.
#[derive(Deserialize)]
struct VerifyRequest {
    provider: String,
    #[serde(default)]
    account_id: Option<String>,
    token: String,
    #[serde(default)]
    requested_leader: Option<String>,
}

#[derive(Serialize)]
struct VerifyResponse {
    account_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    leader: Option<String>,
}

#[derive(Deserialize)]
struct VerifiedIdentityRequest {
    provider: String,
    environment: String,
    external_subject: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    avatar_url: Option<String>,
    #[serde(default)]
    requested_leader: Option<String>,
}

#[derive(Serialize)]
struct VerifiedIdentityResponse {
    account_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    leader: Option<String>,
    account: sow_data::db::PlayerAccount,
}

#[derive(Deserialize)]
struct PlayGamesExchangeRequest {
    server_auth_code: String,
    package_name: String,
}

#[derive(Serialize)]
struct PlayGamesExchangeResponse {
    handoff_token: String,
}

#[derive(Deserialize)]
struct PlayGamesConsumeRequest {
    handoff_token: String,
}

#[derive(Serialize)]
struct PlayGamesIdentityResponse {
    provider: &'static str,
    external_id: String,
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
    name_locked: bool,
    token: String,
}

/// POST /internal/profile/delete — operator-only account erasure for privacy
/// deletion requests (GDPR/CCPA). Same bearer gate as every /internal route
/// and loopback-bound by default; never exposed publicly. Players request
/// deletion via hello@shadowsofwar.io (see Privacy Policy); the operator
/// runs this endpoint and confirms the report.
#[derive(Deserialize)]
struct ProfileDeleteRequest {
    account_id: String,
}

#[derive(Serialize)]
struct ProfileDeleteResponse {
    account_id: String,
    found: bool,
    keys_removed: u64,
    redb_rows_removed: u32,
    analytics_sets_scrubbed: u64,
}

#[derive(Deserialize)]
struct UnlockLeaderRequest {
    public_id: String,
    auth_secret: String,
    leader_id: String,
    #[serde(default)]
    currency: String,
}

#[derive(Deserialize)]
struct UnlockSkinRequest {
    public_id: String,
    auth_secret: String,
    skin_id: String,
}

#[derive(Deserialize)]
struct EquipSkinRequest {
    public_id: String,
    auth_secret: String,
    #[serde(default)]
    skin_id: Option<String>,
}

#[derive(Deserialize)]
struct RevenueCatWebhookRequest {
    event: RevenueCatWebhookEvent,
}

#[derive(Deserialize)]
struct RevenueCatWebhookEvent {
    id: String,
    #[serde(rename = "type")]
    event_type: String,
    app_user_id: String,
    #[serde(default)]
    product_id: Option<String>,
}

async fn handle_store_catalog() -> Json<sow_data::commerce::StoreCatalog> {
    Json(sow_data::commerce::catalog_for_profile(
        &Default::default(),
        &Default::default(),
        0,
        0,
        sow_data::commerce::current_rotation_period(),
    ))
}

/// POST /store/leaders/unlock — spend authoritative laurels on a leader.
async fn handle_unlock_leader(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UnlockLeaderRequest>,
) -> impl IntoResponse {
    let account_id = match state.db.account_id_for_public_id(&payload.public_id) {
        Ok(Some(account_id)) => account_id,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "profile not found".to_string(),
                }),
            )
                .into_response();
        }
        Err(error) => {
            error!("leader unlock profile lookup failed: {error}");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "profile unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };
    if let Err(error) = state
        .db
        .verify_anonymous_secret(&account_id, &payload.auth_secret)
        .await
    {
        return (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })).into_response();
    }
    match state
        .db
        .unlock_leader(&account_id, &payload.leader_id, &payload.currency)
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account.without_auth_secret())).into_response(),
        Err(error) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn handle_unlock_skin(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UnlockSkinRequest>,
) -> impl IntoResponse {
    let account_id = match state.db.account_id_for_public_id(&payload.public_id) {
        Ok(Some(account_id)) => account_id,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "profile not found".to_string(),
                }),
            )
                .into_response();
        }
        Err(error) => {
            error!("skin unlock profile lookup failed: {error}");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "profile unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };
    if let Err(error) = state
        .db
        .verify_anonymous_secret(&account_id, &payload.auth_secret)
        .await
    {
        return (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })).into_response();
    }
    match state
        .db
        .unlock_skin_with_gems(&account_id, &payload.skin_id)
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account.without_auth_secret())).into_response(),
        Err(error) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn handle_equip_skin(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EquipSkinRequest>,
) -> impl IntoResponse {
    let account_id = match state.db.account_id_for_public_id(&payload.public_id) {
        Ok(Some(account_id)) => account_id,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "profile not found".to_string(),
                }),
            )
                .into_response();
        }
        Err(error) => {
            error!("skin equip profile lookup failed: {error}");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "profile unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };
    if let Err(error) = state
        .db
        .verify_anonymous_secret(&account_id, &payload.auth_secret)
        .await
    {
        return (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })).into_response();
    }
    match state
        .db
        .equip_skin(&account_id, payload.skin_id.as_deref())
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account.without_auth_secret())).into_response(),
        Err(error) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn handle_revenuecat_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<RevenueCatWebhookRequest>,
) -> impl IntoResponse {
    let Some(webhook_secret) = state.revenuecat_webhook_secret.as_deref() else {
        error!("RevenueCat webhook rejected: webhook secret is not configured");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "RevenueCat webhook is not configured".to_string(),
            }),
        )
            .into_response();
    };
    let authorized = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == webhook_secret);
    if !authorized {
        warn!("Unauthorized RevenueCat webhook request");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }

    let event = payload.event;
    // Grants flow one way; reversals flow the other. Refund/void/chargeback
    // notifications revoke the bundle amount (floored at zero, deduplicated
    // by event id) so the no-refund economy in the Terms stays real instead
    // of aspirational. Suspension for abuse stays a manual operator call.
    let refund = matches!(
        event.event_type.as_str(),
        "CANCELLATION" | "REFUND" | "VOIDED_PURCHASE"
    );
    if event.event_type != "NON_RENEWING_PURCHASE" && !refund {
        info!(
            "RevenueCat event ignored type={} event={}",
            event.event_type, event.id
        );
        return (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ignored" })),
        )
            .into_response();
    }
    let Some(product_id) = event.product_id.as_deref() else {
        warn!("RevenueCat purchase event {} has no product_id", event.id);
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "RevenueCat event missing product_id".to_string(),
            }),
        )
            .into_response();
    };
    if refund {
        return match state
            .db
            .revoke_revenuecat_gems(&event.id, &event.app_user_id, product_id)
            .await
        {
            Ok((account, true)) => {
                info!(
                    "RevenueCat gems revoked account={} product={} gems={} type={}",
                    account_hint(Some(&account.id)),
                    product_id,
                    account.profile.gems,
                    event.event_type
                );
                (
                    StatusCode::OK,
                    Json(serde_json::json!({ "status": "revoked" })),
                )
                    .into_response()
            }
            Ok((_, false)) => (
                StatusCode::OK,
                Json(serde_json::json!({ "status": "duplicate" })),
            )
                .into_response(),
            Err(error) => {
                error!("RevenueCat gem revocation failed: {error}");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(ErrorResponse {
                        error: "purchase revocation unavailable".to_string(),
                    }),
                )
                    .into_response()
            }
        };
    }
    match state
        .db
        .grant_revenuecat_gems(&event.id, &event.app_user_id, product_id)
        .await
    {
        Ok((account, true)) => {
            info!(
                "RevenueCat gems granted account={} product={} gems={}",
                account_hint(Some(&account.id)),
                product_id,
                account.profile.gems
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({ "status": "granted" })),
            )
                .into_response()
        }
        Ok((_, false)) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "duplicate" })),
        )
            .into_response(),
        Err(error) => {
            error!("RevenueCat gem grant failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "purchase grant unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// Shared ownership-proof gate for self-service account endpoints. The
/// canonical account ID is a lookup key, not a secret — every mutating call
/// must also present the one-time ownership secret minted at creation.
async fn verify_self_service(
    db: &sow_data::db::PlayerDb,
    account_id: &str,
    auth_secret: &str,
) -> Result<(), String> {
    db.verify_anonymous_secret(account_id, auth_secret)
        .await
        .map(|_| ())
}

/// POST /profile/anonymous/report — file a conduct report against another
/// player. Ownership-proofed (reporter secret), rate-limited, and always
/// activates a block. The moderation mailbox receives the report server-side;
/// its address is never exposed to clients.
#[derive(Deserialize)]
struct ReportPlayerRequest {
    account_id: String,
    auth_secret: String,
    reported_public_id: String,
    #[serde(default)]
    match_id: Option<String>,
    reason: String,
    #[serde(default)]
    details: Option<String>,
}

#[derive(Serialize)]
struct ReportPlayerResponse {
    report_id: String,
    blocked: bool,
}

async fn handle_report_player(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ReportPlayerRequest>,
) -> impl IntoResponse {
    let account_id = payload.account_id.trim();
    if let Err(error) = verify_self_service(&state.db, account_id, &payload.auth_secret).await {
        warn!("[moderation] report rejected auth: {error}");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "account ownership proof failed".to_string(),
            }),
        )
            .into_response();
    }
    let reported_public_id = payload.reported_public_id.trim().to_string();
    let reported_account_id = match state.db.account_id_for_public_id(&reported_public_id) {
        Ok(Some(id)) => id,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "reported player not found".to_string(),
                }),
            )
                .into_response();
        }
        Err(error) => {
            error!("[moderation] report lookup failed: {error}");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "report service unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };
    match sow_data::moderation::submit_report(
        &state.db,
        sow_data::moderation::ReportInput {
            reporter_account_id: account_id.to_string(),
            reported_account_id,
            reported_public_id,
            match_id: payload.match_id,
            reason: payload.reason.trim().to_string(),
            details: payload.details,
        },
    )
    .await
    {
        Ok(outcome) => {
            info!(
                "[moderation] report {} filed by {} email_sent={}",
                outcome.report_id,
                account_hint(Some(account_id)),
                outcome.email_sent
            );
            (
                StatusCode::OK,
                Json(ReportPlayerResponse {
                    report_id: outcome.report_id,
                    blocked: outcome.blocked,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let message = error.to_string();
            let status = if message.contains("rate limit") {
                StatusCode::TOO_MANY_REQUESTS
            } else if message.contains("unknown report reason")
                || message.contains("details are required")
                || message.contains("own account")
            {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            };
            warn!("[moderation] report failed: {message}");
            (
                status,
                Json(ErrorResponse {
                    error: "report could not be filed".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// POST /profile/anonymous/blocks — owner-only read of the accounts this
/// account blocked. Clients fetch it at boot to filter chat and presence.
#[derive(Deserialize)]
struct BlocksRequest {
    account_id: String,
    auth_secret: String,
}

#[derive(Serialize)]
struct BlocksResponse {
    blocked_ids: Vec<String>,
}

async fn handle_blocks(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BlocksRequest>,
) -> impl IntoResponse {
    let account_id = payload.account_id.trim();
    if let Err(error) = verify_self_service(&state.db, account_id, &payload.auth_secret).await {
        warn!("[moderation] blocks read rejected auth: {error}");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "account ownership proof failed".to_string(),
            }),
        )
            .into_response();
    }
    match sow_data::moderation::blocked_ids(&state.db, account_id).await {
        Ok(blocked_ids) => (StatusCode::OK, Json(BlocksResponse { blocked_ids })).into_response(),
        Err(error) => {
            error!("[moderation] blocks read failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "block list unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// POST /profile/anonymous/delete — self-service erasure. Same ownership
/// proof as renames and store unlocks, same erasure engine as the operator
/// endpoint. The client wipes local data on success and mints a fresh
/// anonymous account on next boot.
#[derive(Deserialize)]
struct SelfDeleteRequest {
    account_id: String,
    auth_secret: String,
}

#[derive(Serialize)]
struct SelfDeleteResponse {
    deleted: bool,
    keys_removed: u64,
}

async fn handle_self_delete(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SelfDeleteRequest>,
) -> impl IntoResponse {
    let account_id = payload.account_id.trim();
    if let Err(error) = verify_self_service(&state.db, account_id, &payload.auth_secret).await {
        warn!("[privacy] self-delete rejected auth: {error}");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "account ownership proof failed".to_string(),
            }),
        )
            .into_response();
    }
    match state.db.delete_account(account_id).await {
        Ok(report) => {
            info!(
                "[privacy] self-delete ok account={} keys={} sets={}",
                account_hint(Some(&report.account_id)),
                report.keys_removed,
                report.analytics_sets_scrubbed
            );
            (
                StatusCode::OK,
                Json(SelfDeleteResponse {
                    deleted: true,
                    keys_removed: report.keys_removed,
                }),
            )
                .into_response()
        }
        Err(error) => {
            error!("[privacy] self-delete failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "account deletion unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// /profile/anonymous response: the account plus (only when just minted) the
/// one-time ownership secret. Flatten keeps the shape identical to a bare
/// `PlayerAccount` for existing clients — serde ignores the extra field.
#[derive(Serialize)]
struct AnonymousProfileResponse {
    #[serde(flatten)]
    account: sow_data::db::PlayerAccount,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_secret: Option<String>,
}

/// POST /internal/verify
async fn handle_internal_verify(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<VerifyRequest>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        warn!("Unauthorized access attempt to /internal/verify");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }
    let provider = payload.provider.trim();
    let result: Result<String, String> = match provider {
        "anonymous" => match (payload.account_id.as_deref(), payload.token.as_str()) {
            (Some(account_id), token) if !account_id.trim().is_empty() => {
                state.db.verify_anonymous_secret(account_id, token).await
            }
            _ => Err("anonymous verification requires account_id and token".to_string()),
        },
        "wou" | "wou_id" | "world_of_unreal" | "crazygames" | "playgames" => {
            Err("external identities must be verified by sow-server".to_string())
        }
        other => Err(format!("unsupported provider: {other}")),
    };
    match result {
        Ok(account_id) => {
            let leader = match payload.requested_leader.as_deref() {
                Some(requested) => match state
                    .db
                    .resolve_leader_for_account(&account_id, Some(requested))
                    .await
                {
                    Ok(resolution) => {
                        Some(sow_data::commerce::leader_wire_id(resolution.resolved).to_string())
                    }
                    Err(error) => {
                        error!(
                            "[identity] leader resolution failed account={} error={error}",
                            account_hint(Some(&account_id))
                        );
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ErrorResponse {
                                error: "leader resolution unavailable".to_string(),
                            }),
                        )
                            .into_response();
                    }
                },
                None => None,
            };
            info!(
                "[identity] verify ok provider={provider} account={}",
                account_hint(Some(&account_id))
            );
            (StatusCode::OK, Json(VerifyResponse { account_id, leader })).into_response()
        }
        Err(e) => {
            warn!("[identity] verify failed provider={provider}: {e}");
            (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error: e })).into_response()
        }
    }
}

/// POST /internal/identity/resolve — persist only an identity already
/// verified by sow-server. Provider tokens never enter this service.
async fn handle_internal_identity_resolve(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<VerifiedIdentityRequest>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        warn!("Unauthorized access attempt to /internal/identity/resolve");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }
    if payload.provider.trim().is_empty()
        || payload.environment.trim().is_empty()
        || payload.external_subject.trim().is_empty()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "verified identity fields are incomplete".to_string(),
            }),
        )
            .into_response();
    }
    let account = match state
        .db
        .get_or_create_with_environment(
            payload.provider.trim().to_string(),
            payload.environment.trim().to_string(),
            payload.external_subject.trim().to_string(),
        )
        .await
    {
        Ok(account) => account,
        Err(error) => {
            error!("verified identity persistence failed: {error}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "identity persistence unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };
    let leader = match payload.requested_leader.as_deref() {
        Some(requested) => match state
            .db
            .resolve_leader_for_account(&account.id, Some(requested))
            .await
        {
            Ok(resolution) => {
                Some(sow_data::commerce::leader_wire_id(resolution.resolved).to_string())
            }
            Err(error) => {
                error!("verified identity leader resolution failed: {error}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "leader resolution unavailable".to_string(),
                    }),
                )
                    .into_response();
            }
        },
        None => None,
    };
    // These are deliberately accepted at the boundary for future profile
    // enrichment. PlayerAccount currently owns only its display name.
    let _ = (payload.display_name, payload.avatar_url);
    (
        StatusCode::OK,
        Json(VerifiedIdentityResponse {
            account_id: account.id.clone(),
            leader,
            account: account.without_auth_secret(),
        }),
    )
        .into_response()
}

async fn handle_internal_profile_delete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ProfileDeleteRequest>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        warn!("Unauthorized access attempt to /internal/profile/delete");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }
    let account_id = payload.account_id.trim().to_string();
    match state.db.delete_account(&account_id).await {
        Ok(report) => {
            info!(
                "[privacy] account erased account={} keys={} redb={} sets={}",
                account_hint(Some(&report.account_id)),
                report.keys_removed,
                report.redb_rows_removed,
                report.analytics_sets_scrubbed
            );
            (
                StatusCode::OK,
                Json(ProfileDeleteResponse {
                    account_id: report.account_id,
                    found: report.found,
                    keys_removed: report.keys_removed,
                    redb_rows_removed: report.redb_rows_removed,
                    analytics_sets_scrubbed: report.analytics_sets_scrubbed,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let message = error.to_string();
            let status = if message.contains("invalid account_id") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            warn!("[privacy] account erase failed: {message}");
            (
                status,
                Json(ErrorResponse {
                    error: "account erasure unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

#[derive(Serialize)]
struct BotPoolSeedResponse {
    account_ids: Vec<String>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn account_hint(account_id: Option<&str>) -> String {
    account_id
        .map(|id| id.chars().take(8).collect::<String>())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| "none".to_string())
}

fn identity_request_hint(headers: &HeaderMap) -> String {
    headers
        .get("x-sow-identity-request")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .unwrap_or("missing")
        .to_string()
}

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    info!("Starting SOW-DATABASE microservice with Valkey...");

    // Read environment variables
    let port: u16 = std::env::var("SOW_DB_PORT")
        .unwrap_or_else(|_| "25585".to_string())
        .parse()
        .unwrap_or(25585);

    let valkey_url = std::env::var("SOW_VALKEY_URL")
        .or_else(|_| std::env::var("SOW_REDIS_URL"))
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    let secret_token = std::env::var("SOW_DB_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .expect("SOW_DB_SECRET must be set; refusing insecure default");

    let revenuecat_webhook_secret = std::env::var("SOW_REVENUECAT_WEBHOOK_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty());

    let crazygames_api_key = std::env::var("CRAZYGAMES_API_KEY").ok();

    let sanitized_valkey = if let Some(pos) = valkey_url.find('@') {
        if let Some(scheme_pos) = valkey_url.find("://") {
            format!(
                "{}***@{}",
                &valkey_url[..scheme_pos + 3],
                &valkey_url[pos + 1..]
            )
        } else {
            format!("***@{}", &valkey_url[pos + 1..])
        }
    } else {
        valkey_url.clone()
    };

    info!(
        "Config - Port: {}, Valkey: {}, Secret: [REDACTED], CG API Key Configured: {}, RevenueCat Webhook Configured: {}",
        port,
        sanitized_valkey,
        crazygames_api_key.is_some(),
        revenuecat_webhook_secret.is_some()
    );

    // Open REDB persistent database
    let redb_path =
        std::env::var("SOW_REDB_PATH").unwrap_or_else(|_| "sow_metadata.redb".to_string());
    info!("Opening persistent REDB database at {}", redb_path);
    if let Some(dir) = std::path::Path::new(&redb_path).parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir).expect("Failed to create REDB data directory");
    }
    let redb_db =
        sow_data::init_database(&redb_path).expect("Failed to initialize REDB metadata database");
    let redb_db_arc = Arc::new(redb_db);

    // Seed Valkey RAM from REDB on boot
    info!("Seeding Valkey RAM cache from REDB...");
    if let Err(e) = sow_data::metadata_db::seed_valkey_from_redb(&redb_db_arc, &valkey_url) {
        error!("Failed to seed Valkey on startup: {e}");
    }

    // Initialize database connector
    let player_db = PlayerDb::new(
        &valkey_url,
        crazygames_api_key,
        Some(Arc::clone(&redb_db_arc)),
    );
    player_db
        .ensure_current_season()
        .expect("Failed to initialize current profile season");
    match player_db.backfill_public_profiles().await {
        Ok(migrated) => info!("Public profile index ready; migrated {migrated} legacy accounts"),
        Err(error) => panic!("Failed to backfill public profile index: {error}"),
    }

    let default_analytics_dir = std::path::Path::new(&redb_path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.join("analytics").to_string_lossy().into_owned())
        .unwrap_or_else(|| "analytics".to_string());

    let analytics_dir = std::env::var("SOW_ANALYTICS_DIR").unwrap_or(default_analytics_dir);
    let event_sink = sow_data::events::EventSink::new(&analytics_dir).unwrap_or_else(|error| {
        panic!("Failed to initialize analytics event sink at {analytics_dir}: {error}");
    });

    let state = Arc::new(AppState {
        db: player_db,
        secret_token,
        revenuecat_webhook_secret,
        redb_path: redb_path.clone(),
        events: std::sync::Mutex::new(event_sink),
        playgames_handoffs: std::sync::Mutex::new(HashMap::new()),
        playgames_sessions: std::sync::Mutex::new(HashMap::new()),
        playgames_access_tokens: std::sync::Mutex::new(HashMap::new()),
    });

    // Configure CORS for web portal compatibility
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::HeaderName::from_static("x-platform-auth"),
            header::HeaderName::from_static("x-sow-identity-request"),
        ]);

    // Define router
    let app = Router::new()
        .route("/healthz", get(handle_healthz))
        .route("/event", post(handle_event))
        .route("/internal/analytics", get(handle_internal_analytics))
        .route("/profile", get(handle_get_profile))
        .route("/auth/playgames/exchange", post(handle_playgames_exchange))
        .route("/auth/playgames/consume", post(handle_playgames_consume))
        .route("/store/catalog", get(handle_store_catalog))
        .route("/store/leaders/unlock", post(handle_unlock_leader))
        .route("/store/skins/unlock", post(handle_unlock_skin))
        .route("/store/skins/equip", post(handle_equip_skin))
        .route(
            "/internal/revenuecat/webhook",
            post(handle_revenuecat_webhook),
        )
        .route(
            "/internal/revenuecat/webhook/stripe",
            post(handle_revenuecat_webhook),
        )
        .route("/profiles/search", get(handle_public_profile_search))
        .route("/profiles/{public_id}", get(handle_public_profile))
        .route(
            "/profiles/{public_id}/matches",
            get(handle_public_match_history),
        )
        .route(
            "/profiles/{public_id}/seasons",
            get(handle_public_profile_seasons),
        )
        .route("/matches/{match_id}", get(handle_public_match_detail))
        .route("/seasons/current", get(handle_current_season))
        .route(
            "/seasons/{season_id}/leaderboard",
            get(handle_public_leaderboard),
        )
        .route("/profile/anonymous", post(handle_anonymous_profile))
        .route(
            "/profile/anonymous/name",
            post(handle_anonymous_display_name),
        )
        .route(
            "/profile/anonymous/tutorial-complete",
            post(handle_anonymous_tutorial_complete),
        )
        .route("/profile/anonymous/report", post(handle_report_player))
        .route("/profile/anonymous/blocks", post(handle_blocks))
        .route("/profile/anonymous/delete", post(handle_self_delete))
        .route("/match/start", post(handle_match_start))
        .route("/internal/match-finalize", post(handle_match_finalize))
        .route("/internal/save", post(handle_direct_save))
        .route("/internal/stats", get(handle_internal_stats))
        .route("/internal/verify", post(handle_internal_verify))
        .route(
            "/internal/identity/resolve",
            post(handle_internal_identity_resolve),
        )
        .route(
            "/internal/profile/delete",
            post(handle_internal_profile_delete),
        )
        .route("/internal/bot-pool/seed", post(handle_bot_pool_seed))
        .layer(DefaultBodyLimit::max(MAX_REPLAY_REQUEST_BYTES))
        .layer(cors)
        .with_state(state);

    // Bind to loopback by default. Public exposure must be an explicit
    // deployment decision made with SOW_DB_LISTEN.
    let addr: SocketAddr = std::env::var("SOW_DB_LISTEN")
        .unwrap_or_else(|_| format!("127.0.0.1:{port}"))
        .parse()
        .expect("SOW_DB_LISTEN must be a valid socket address");
    info!("SOW-DATABASE serving on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handle_public_profile_search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PublicSearchQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(20).clamp(1, 20);
    let cursor = query.cursor.unwrap_or(0).min(10_000);
    let query_text = query.q.as_deref().unwrap_or("");
    match state
        .db
        .search_public_profiles(query_text, cursor.saturating_add(limit))
        .await
    {
        Ok(mut profiles) => {
            if cursor < profiles.len() {
                profiles.drain(..cursor);
            } else {
                profiles.clear();
            }
            profiles.truncate(limit);
            let has_next = profiles.len() == limit;
            let next_cursor = has_next.then_some(cursor.saturating_add(limit));
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "items": profiles,
                    "next_cursor": next_cursor
                })),
            )
                .into_response()
        }
        Err(error) => {
            error!("public profile search failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "profiles unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_public_profile(
    State(state): State<Arc<AppState>>,
    Path(public_id): Path<String>,
) -> impl IntoResponse {
    match state.db.public_profile(&public_id).await {
        Ok(Some(profile)) => (StatusCode::OK, Json(profile)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "profile not found".to_string(),
            }),
        )
            .into_response(),
        Err(error) => {
            error!("public profile lookup failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "profiles unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_public_match_history(
    State(state): State<Arc<AppState>>,
    Path(public_id): Path<String>,
    Query(query): Query<PublicHistoryQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let cursor = query.cursor.unwrap_or(0).min(10_000);
    let queue = query.queue.as_deref();
    let mode = query.mode.as_deref();
    match state
        .db
        .public_match_history(&public_id, cursor, limit, queue, mode)
        .await
    {
        Ok(Some(history)) => {
            let has_next = history.len() > limit;
            let next_cursor = if has_next {
                Some(cursor.saturating_add(limit))
            } else {
                None
            };
            let items = history.into_iter().take(limit).collect::<Vec<_>>();
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "items": items,
                    "next_cursor": next_cursor,
                })),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "profile not found".to_string(),
            }),
        )
            .into_response(),
        Err(error) => {
            error!("public match history failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "match history unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_public_match_detail(
    State(state): State<Arc<AppState>>,
    Path(match_id): Path<String>,
) -> impl IntoResponse {
    match state.db.public_match_detail(&match_id) {
        Ok(Some(detail)) => (StatusCode::OK, Json(detail)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "match not found".to_string(),
            }),
        )
            .into_response(),
        Err(error) => {
            error!("public match detail failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "match unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_public_profile_seasons(
    State(state): State<Arc<AppState>>,
    Path(public_id): Path<String>,
) -> impl IntoResponse {
    match state.db.public_ratings(&public_id).await {
        Ok(Some(ratings)) => (
            StatusCode::OK,
            Json(serde_json::json!({ "items": ratings })),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "profile not found".to_string(),
            }),
        )
            .into_response(),
        Err(error) => {
            error!("public profile seasons failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "seasons unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_current_season(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.db.seasons() {
        Ok(seasons) => {
            let current = seasons
                .iter()
                .find(|season| season.id == sow_data::profile::CURRENT_SEASON_ID)
                .cloned();
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "season": current,
                    "items": seasons,
                })),
            )
                .into_response()
        }
        Err(error) => {
            error!("current season lookup failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "seasons unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn handle_public_leaderboard(
    State(state): State<Arc<AppState>>,
    Path(season_id): Path<u32>,
    Query(query): Query<PublicLeaderboardQuery>,
) -> impl IntoResponse {
    if season_id != sow_data::profile::CURRENT_SEASON_ID {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "season not found".to_string(),
            }),
        )
            .into_response();
    }
    let queue = query.queue.as_deref().unwrap_or("Matchmaking");
    let mode = query.mode.as_deref().unwrap_or("FFA");
    let limit = query.limit.unwrap_or(100).clamp(1, 100);
    let cursor = query.cursor.unwrap_or(0).min(10_000);
    match state
        .db
        .public_leaderboard(queue, mode, cursor.saturating_add(limit))
        .await
    {
        Ok(mut items) => {
            if cursor < items.len() {
                items.drain(..cursor);
            } else {
                items.clear();
            }
            items.truncate(limit);
            let has_next = items.len() == limit;
            let next_cursor = has_next.then_some(cursor.saturating_add(limit));
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "season_id": season_id,
                    "queue": queue,
                    "mode": mode,
                    "items": items,
                    "next_cursor": next_cursor,
                })),
            )
                .into_response()
        }
        Err(error) => {
            error!("public leaderboard lookup failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "leaderboard unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// Liveness probe for the pipeline and service supervisor. It deliberately
/// performs no profile lookup and emits no authentication warning.
async fn handle_healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
}

const MAX_EVENT_BATCH: usize = 100;

#[derive(Deserialize)]
struct EventBatchRequest {
    events: Vec<sow_data::events::AnalyticsEvent>,
}

/// Append one validated event line to the daily JSONL sink.
fn emit_event_line(
    state: &AppState,
    name: &str,
    account_id: Option<&str>,
    props: serde_json::Value,
) {
    let ts_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let mut event = serde_json::json!({
        "v": sow_data::events::SCHEMA_VERSION,
        "name": name,
        "ts_ms": ts_ms,
        "session_id": "server",
        "platform": "server",
    });
    if let Some(account_id) = account_id {
        event["account_id"] = serde_json::Value::String(account_id.to_string());
    }
    if !props.is_null() {
        event["props"] = props;
    }
    if let Err(e) = state
        .events
        .lock()
        .map(|mut sink| sink.append_line(&event.to_string()))
    {
        warn!("analytics append failed for {name}: {e:?}");
    }
}

/// POST /event — public anonymous product-analytics ingestion. Valid events go
/// to the durable JSONL sink; bot accounts are dropped and never marked active.
async fn handle_event(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EventBatchRequest>,
) -> impl IntoResponse {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let mut accepted = 0usize;
    let mut rejected = 0usize;
    for event in payload.events.into_iter().take(MAX_EVENT_BATCH) {
        if let Err(reason) = event.validate(now_ms) {
            warn!("Dropping analytics event '{}': {reason}", event.name);
            rejected += 1;
            continue;
        }
        if let Some(account_id) = &event.account_id {
            match state.db.is_bot_account_checked(account_id).await {
                Ok(true) => {
                    rejected += 1;
                    continue;
                }
                Ok(false) => {}
                Err(error) => {
                    rejected += 1;
                    warn!("analytics bot-pool lookup failed: {error}");
                    continue;
                }
            }
        }
        let write_result = {
            let Ok(mut sink) = state.events.lock() else {
                rejected += 1;
                continue;
            };
            serde_json::to_string(&event)
                .map_err(|e| e.to_string())
                .and_then(|line| sink.append_line(&line).map_err(|e| e.to_string()))
        };
        match write_result {
            Ok(()) => {
                if let Err(e) = state
                    .db
                    .record_product_event(&event.name, event.account_id.as_deref())
                    .await
                {
                    warn!("analytics counter write failed for {}: {e}", event.name);
                }
                accepted += 1;
            }
            Err(e) => {
                rejected += 1;
                warn!("analytics write failed: {e}");
            }
        }
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({ "accepted": accepted, "rejected": rejected })),
    )
}

#[derive(Deserialize)]
struct AnalyticsQuery {
    days: Option<u32>,
}

/// GET /internal/analytics — operator-only funnel and retention snapshot.
async fn handle_internal_analytics(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<AnalyticsQuery>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }
    match state.db.analytics_summary(query.days.unwrap_or(30)).await {
        Ok(summary) => (StatusCode::OK, Json(summary)).into_response(),
        Err(error) => {
            error!("analytics summary failed: {error}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "analytics unavailable".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// POST /profile/anonymous — load the canonical anonymous account and issue
/// one for a new browser profile. The initial display name is stored only when
/// the account is created; later renames use the explicit name endpoint.
async fn handle_anonymous_profile(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<AnonymousProfileRequest>,
) -> impl IntoResponse {
    let request_id = identity_request_hint(&headers);
    info!(
        "[identity] profile request id={request_id} account={} requested_name_len={}",
        account_hint(payload.account_id.as_deref()),
        payload
            .display_name
            .as_deref()
            .map(|name| name.chars().count())
            .unwrap_or(0)
    );
    match state
        .db
        .get_or_create_anonymous(
            payload.account_id.as_deref(),
            payload.display_name.as_deref(),
            payload.auth_secret.as_deref(),
        )
        .await
    {
        Ok(account) => {
            // Mint the ownership secret on first sight (new account or
            // pre-secret legacy account); the plaintext travels exactly once.
            let revealed_secret = state
                .db
                .ensure_auth_secret(&account.id)
                .await
                .map_err(|e| {
                    warn!(
                        "[identity] secret mint failed for {}: {e}",
                        account_hint(Some(&account.id))
                    );
                })
                .ok()
                .flatten();
            info!(
                "[identity] profile ack id={request_id} account={} name_len={}",
                account_hint(Some(&account.id)),
                account.display_name.chars().count()
            );
            (
                StatusCode::OK,
                Json(AnonymousProfileResponse {
                    account: account.without_auth_secret(),
                    auth_secret: revealed_secret,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let message = error.to_string();
            let status = if message.contains("invalid secret") {
                StatusCode::UNAUTHORIZED
            } else if message.contains("account_id must") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::NOT_FOUND
            };
            warn!(
                "[identity] profile failed id={request_id} account={} status={} error={}",
                account_hint(payload.account_id.as_deref()),
                status,
                message
            );
            (status, Json(ErrorResponse { error: message })).into_response()
        }
    }
}

/// POST /profile/anonymous/name — persist a rename for an anonymous account.
async fn handle_anonymous_display_name(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<AnonymousDisplayNameRequest>,
) -> impl IntoResponse {
    let request_id = identity_request_hint(&headers);
    info!(
        "[identity] rename request id={request_id} account={} requested_name_len={}",
        account_hint(Some(&payload.account_id)),
        payload.display_name.chars().count()
    );
    match state
        .db
        .update_anonymous_display_name(
            &payload.account_id,
            &payload.display_name,
            &payload.auth_secret,
        )
        .await
    {
        Ok(account) => {
            info!(
                "[identity] rename ack id={request_id} account={} name_len={}",
                account_hint(Some(&account.id)),
                account.display_name.chars().count()
            );
            (StatusCode::OK, Json(account.without_auth_secret())).into_response()
        }
        Err(error) => {
            let message = error.to_string();
            let status = if message.contains("invalid secret") {
                StatusCode::UNAUTHORIZED
            } else if message.contains("account_id must") || message.contains("display_name") {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::NOT_FOUND
            };
            warn!(
                "[identity] rename failed id={request_id} account={} status={} error={}",
                account_hint(Some(&payload.account_id)),
                status,
                message
            );
            (status, Json(ErrorResponse { error: message })).into_response()
        }
    }
}

/// POST /profile/anonymous/tutorial-complete — authenticate the anonymous
/// account and grant the onboarding reward exactly once.
async fn handle_anonymous_tutorial_complete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<TutorialCompleteRequest>,
) -> impl IntoResponse {
    let request_id = identity_request_hint(&headers);
    let account_id = match state
        .db
        .verify_anonymous_secret(&payload.account_id, &payload.auth_secret)
        .await
    {
        Ok(account_id) => account_id,
        Err(error) => {
            warn!(
                "[tutorial] completion rejected id={request_id} account={} error={error}",
                account_hint(Some(&payload.account_id))
            );
            return (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })).into_response();
        }
    };

    match state.db.complete_anonymous_tutorial(&account_id).await {
        Ok(account) => {
            info!(
                "[tutorial] completion accepted id={request_id} account={} completed={}",
                account_hint(Some(&account.id)),
                account.profile.intro_completed
            );
            (StatusCode::OK, Json(account.without_auth_secret())).into_response()
        }
        Err(error) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
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

fn random_playgames_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

async fn handle_playgames_exchange(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PlayGamesExchangeRequest>,
) -> impl IntoResponse {
    if payload.package_name != "com.shadowsofwar"
        && payload.package_name != "com.shadowsofwar.debug"
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "unsupported Android package".to_string(),
            }),
        )
            .into_response();
    }
    if payload.server_auth_code.trim().is_empty() || payload.server_auth_code.len() > 4096 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid Play Games server auth code".to_string(),
            }),
        )
            .into_response();
    }

    let client_id = match std::env::var("SOW_PLAY_GAMES_WEB_CLIENT_ID") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            error!("Play Games server client ID is not configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Play Games server access is not configured".to_string(),
                }),
            )
                .into_response();
        }
    };
    let client_secret = match std::env::var("SOW_PLAY_GAMES_WEB_CLIENT_SECRET") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            error!("Play Games server client secret is not configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Play Games server access is not configured".to_string(),
                }),
            )
                .into_response();
        }
    };

    let client = reqwest::Client::new();
    let token_response = match client
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
    {
        Ok(response) if response.status().is_success() => {
            match response.json::<PlayGamesOAuthTokenResponse>().await {
                Ok(value) if !value.access_token.is_empty() => value,
                Ok(_) => {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(ErrorResponse {
                            error: "Play Games token response was empty".to_string(),
                        }),
                    )
                        .into_response();
                }
                Err(error) => {
                    warn!("Play Games token response unreadable: {error}");
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(ErrorResponse {
                            error: "Play Games token exchange failed".to_string(),
                        }),
                    )
                        .into_response();
                }
            }
        }
        Ok(response) => {
            warn!(
                "Play Games token exchange rejected: HTTP {}",
                response.status()
            );
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Play Games authentication was rejected".to_string(),
                }),
            )
                .into_response();
        }
        Err(error) => {
            warn!("Play Games token exchange failed: {error}");
            return (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: "Play Games authentication is temporarily unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };

    let access_token = token_response.access_token.clone();
    let player = match client
        .get("https://games.googleapis.com/games/v1/players/me")
        .bearer_auth(&access_token)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            match response.json::<PlayGamesPlayerResponse>().await {
                Ok(player) => player,
                Err(error) => {
                    warn!("Play Games player response unreadable: {error}");
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(ErrorResponse {
                            error: "Play Games player verification failed".to_string(),
                        }),
                    )
                        .into_response();
                }
            }
        }
        Ok(response) => {
            warn!(
                "Play Games player verification rejected: HTTP {}",
                response.status()
            );
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Play Games player verification was rejected".to_string(),
                }),
            )
                .into_response();
        }
        Err(error) => {
            warn!("Play Games player verification failed: {error}");
            return (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: "Play Games player verification is temporarily unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };

    if player.player_id.trim().is_empty() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Play Games response did not include a player ID".to_string(),
            }),
        )
            .into_response();
    }

    let account = match state
        .db
        .get_or_create("playgames_android".to_string(), player.player_id.clone())
        .await
    {
        Ok(account) => account,
        Err(error) => {
            error!("Play Games account lookup failed: {error}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Play Games account lookup failed".to_string(),
                }),
            )
                .into_response();
        }
    };

    let token_ttl = token_response
        .expires_in
        .unwrap_or(3600)
        .saturating_sub(30)
        .max(60);
    let Ok(mut access_tokens) = state.playgames_access_tokens.lock() else {
        error!("Play Games access-token store is poisoned");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Play Games session unavailable".to_string(),
            }),
        )
            .into_response();
    };
    access_tokens.insert(
        account.id.clone(),
        PlayGamesAccessToken {
            access_token,
            expires_at: Instant::now() + Duration::from_secs(token_ttl),
        },
    );

    let handoff_token = random_playgames_token();
    let handoff = PlayGamesHandoff {
        expires_at: Instant::now() + PLAYGAMES_HANDOFF_TTL,
        account_id: account.id,
        external_id: player.player_id,
        display_name: player.display_name,
        avatar_url: player.avatar_url,
    };
    let Ok(mut handoffs) = state.playgames_handoffs.lock() else {
        error!("Play Games handoff store is poisoned");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Play Games handoff unavailable".to_string(),
            }),
        )
            .into_response();
    };
    let now = Instant::now();
    handoffs.retain(|_, value| value.expires_at > now);
    handoffs.insert(handoff_token.clone(), handoff);

    (
        StatusCode::OK,
        Json(PlayGamesExchangeResponse { handoff_token }),
    )
        .into_response()
}

async fn handle_playgames_consume(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PlayGamesConsumeRequest>,
) -> impl IntoResponse {
    let handoff = {
        let Ok(mut handoffs) = state.playgames_handoffs.lock() else {
            error!("Play Games handoff store is poisoned");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Play Games handoff unavailable".to_string(),
                }),
            )
                .into_response();
        };
        let now = Instant::now();
        handoffs.retain(|_, value| value.expires_at > now);
        handoffs.remove(payload.handoff_token.trim())
    };

    let Some(handoff) = handoff else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Play Games handoff expired or already used".to_string(),
            }),
        )
            .into_response();
    };

    let token = random_playgames_token();
    let Ok(mut sessions) = state.playgames_sessions.lock() else {
        error!("Play Games session store is poisoned");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Play Games session unavailable".to_string(),
            }),
        )
            .into_response();
    };
    let now = Instant::now();
    sessions.retain(|_, value| value.expires_at > now);
    sessions.insert(
        token.clone(),
        PlayGamesSession {
            expires_at: now + PLAYGAMES_SESSION_TTL,
            account_id: handoff.account_id,
            external_id: handoff.external_id.clone(),
        },
    );

    (
        StatusCode::OK,
        Json(PlayGamesIdentityResponse {
            provider: "playgames",
            external_id: handoff.external_id,
            display_name: handoff.display_name,
            avatar_url: handoff.avatar_url,
            name_locked: true,
            token,
        }),
    )
        .into_response()
}

impl AppState {
    async fn sync_playgames_match_event(&self, participants: &[String]) -> Result<(), String> {
        let event_id = std::env::var("SOW_PLAY_GAMES_MATCH_EVENT_ID")
            .unwrap_or_default()
            .trim()
            .to_string();
        if event_id.is_empty() || participants.is_empty() {
            return Ok(());
        }

        let now = Instant::now();
        let tokens = {
            let mut access_tokens = self
                .playgames_access_tokens
                .lock()
                .map_err(|_| "Play Games access-token store is poisoned".to_string())?;
            access_tokens.retain(|_, value| value.expires_at > now);
            participants
                .iter()
                .filter_map(|account_id| {
                    access_tokens
                        .get(account_id)
                        .map(|value| (account_id.clone(), value.access_token.clone()))
                })
                .collect::<Vec<_>>()
        };
        if tokens.is_empty() {
            return Ok(());
        }

        let body = serde_json::json!({
            "kind": "games#eventRecordRequest",
            "requestId": rand::random::<u64>().to_string(),
            "currentTimeMillis": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .to_string(),
            "events": [{
                "definitionId": event_id,
                "updateCount": "1"
            }]
        });
        let client = reqwest::Client::new();
        let mut first_error = None;
        for (account_id, access_token) in tokens {
            match client
                .post("https://games.googleapis.com/games/v1/events")
                .bearer_auth(access_token)
                .json(&body)
                .timeout(Duration::from_secs(5))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    info!(
                        "Play Games match event recorded for account={}",
                        account_hint(Some(&account_id))
                    );
                }
                Ok(response) => {
                    let message = format!(
                        "HTTP {} for account={}",
                        response.status(),
                        account_hint(Some(&account_id))
                    );
                    error!("Play Games event request rejected: {message}");
                    first_error.get_or_insert(message);
                }
                Err(error) => {
                    let message =
                        format!("{} for account={}", error, account_hint(Some(&account_id)));
                    error!("Play Games event request failed: {message}");
                    first_error.get_or_insert(message);
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    async fn sync_playgames_match_outcome(
        &self,
        outcome: &PlayGamesMatchOutcome,
    ) -> Result<(), String> {
        let env_value = |name: &str| std::env::var(name).unwrap_or_default().trim().to_string();
        let first_victory = env_value("SOW_PLAY_GAMES_FIRST_VICTORY_ACHIEVEMENT_ID");
        let battle_hardened = env_value("SOW_PLAY_GAMES_BATTLE_HARDENED_ACHIEVEMENT_ID");
        let victory_march = env_value("SOW_PLAY_GAMES_VICTORY_MARCH_ACHIEVEMENT_ID");
        let laurel_hoard = env_value("SOW_PLAY_GAMES_LAUREL_HOARD_ACHIEVEMENT_ID");
        let first_command = env_value("SOW_PLAY_GAMES_FIRST_COMMAND_ACHIEVEMENT_ID");
        let commander_victorious = env_value("SOW_PLAY_GAMES_COMMANDER_VICTORIOUS_ACHIEVEMENT_ID");
        let veteran_commander = env_value("SOW_PLAY_GAMES_VETERAN_COMMANDER_ACHIEVEMENT_ID");
        let banner_collector = env_value("SOW_PLAY_GAMES_BANNER_COLLECTOR_ACHIEVEMENT_ID");
        let leader_path = env_value("SOW_PLAY_GAMES_LEADER_PATH_ACHIEVEMENT_ID");
        let leaderboard_id = env_value("SOW_PLAY_GAMES_VICTORIES_LEADERBOARD_ID");
        if first_victory.is_empty()
            && battle_hardened.is_empty()
            && victory_march.is_empty()
            && laurel_hoard.is_empty()
            && first_command.is_empty()
            && commander_victorious.is_empty()
            && veteran_commander.is_empty()
            && banner_collector.is_empty()
            && leader_path.is_empty()
            && leaderboard_id.is_empty()
        {
            return Ok(());
        }

        let access_token = {
            let mut access_tokens = self
                .playgames_access_tokens
                .lock()
                .map_err(|_| "Play Games access-token store is poisoned".to_string())?;
            access_tokens.retain(|_, value| value.expires_at > Instant::now());
            access_tokens
                .get(&outcome.account_id)
                .map(|value| value.access_token.clone())
        };
        let Some(access_token) = access_token else {
            return Ok(());
        };

        let client = reqwest::Client::new();
        let account_hint = account_hint(Some(&outcome.account_id));
        let mut actions = Vec::new();
        let add_unlock =
            |actions: &mut Vec<(String, Option<(&'static str, String)>, &'static str)>,
             id: &str,
             label: &'static str| {
                if !id.is_empty() {
                    actions.push((
                        format!("https://games.googleapis.com/games/v1/achievements/{id}/unlock"),
                        None,
                        label,
                    ));
                }
            };
        let add_increment =
            |actions: &mut Vec<(String, Option<(&'static str, String)>, &'static str)>,
             id: &str,
             steps: u64,
             label: &'static str| {
                if !id.is_empty() && steps > 0 {
                    actions.push((
                        format!(
                            "https://games.googleapis.com/games/v1/achievements/{id}/increment"
                        ),
                        Some(("steps", steps.to_string())),
                        label,
                    ));
                }
            };

        add_increment(&mut actions, &battle_hardened, 1, "Battle Hardened");
        add_increment(&mut actions, &leader_path, 1, "Leader Path");
        add_increment(
            &mut actions,
            &laurel_hoard,
            outcome.laurels_earned,
            "Laurel Hoard",
        );
        if outcome.leader_matches_played >= 1 {
            add_unlock(&mut actions, &first_command, "First Command");
        }
        if outcome.won {
            add_unlock(&mut actions, &first_victory, "First Victory");
            add_unlock(&mut actions, &commander_victorious, "Commander Victorious");
            add_increment(&mut actions, &victory_march, 1, "Victory March");
        }
        if outcome.leader_wins >= 10 {
            add_unlock(&mut actions, &veteran_commander, "Veteran Commander");
        }
        if outcome.distinct_leaders >= 5 {
            add_unlock(&mut actions, &banner_collector, "Banner Collector");
        }
        if outcome.won && !leaderboard_id.is_empty() {
            actions.push((
                format!(
                    "https://games.googleapis.com/games/v1/leaderboards/{leaderboard_id}/scores"
                ),
                Some(("score", outcome.wins.to_string())),
                "Victories leaderboard",
            ));
        }

        let mut first_error = None;
        for (url, query, label) in actions {
            let mut request = client
                .post(url)
                .bearer_auth(&access_token)
                .timeout(Duration::from_secs(5));
            if let Some(query) = query {
                request = request.query(&[query]);
            }
            match request.send().await {
                Ok(response) if response.status().is_success() => {
                    info!("Play Games {label} synced for account={account_hint}");
                }
                Ok(response) => {
                    let message = format!(
                        "HTTP {} syncing {label} for account={account_hint}",
                        response.status()
                    );
                    error!("Play Games request rejected: {message}");
                    first_error.get_or_insert(message);
                }
                Err(error) => {
                    let message = format!("{error} syncing {label} for account={account_hint}");
                    error!("Play Games request failed: {message}");
                    first_error.get_or_insert(message);
                }
            }
        }

        first_error.map_or(Ok(()), Err)
    }

    fn verify_playgames_session(
        &self,
        external_id: Option<&str>,
        token: &str,
    ) -> Result<VerifiedPlayGamesIdentity, String> {
        let Ok(mut sessions) = self.playgames_sessions.lock() else {
            return Err("Play Games session store is poisoned".to_string());
        };
        let now = Instant::now();
        sessions.retain(|_, value| value.expires_at > now);
        let Some(session) = sessions.get(token.trim()) else {
            return Err("Play Games session expired or invalid".to_string());
        };
        if external_id.is_some_and(|value| value != session.external_id) {
            return Err("Play Games player mismatch".to_string());
        }
        Ok(VerifiedPlayGamesIdentity {
            account_id: session.account_id.clone(),
            external_id: session.external_id.clone(),
        })
    }
}

fn platform_auth_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-platform-auth")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

async fn resolve_external_id(
    provider: &str,
    external_id: &str,
    auth_token: Option<&str>,
) -> Result<String, String> {
    if provider == "wou" || provider == "wou_id" || provider == "world_of_unreal" {
        let Some(token) = auth_token else {
            return Err("WOU-ID requests require X-Platform-Auth token".into());
        };
        let wou_url =
            std::env::var("WOU_ID_URL").unwrap_or_else(|_| "http://127.0.0.1:25570".into());
        let client = if let Ok(resolve_ip) = std::env::var("WOU_ID_RESOLVE_IP") {
            let host = reqwest::Url::parse(&wou_url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned))
                .ok_or_else(|| "WOU_ID_URL has no valid hostname".to_string())?;
            let port = reqwest::Url::parse(&wou_url)
                .ok()
                .and_then(|url| url.port_or_known_default())
                .unwrap_or(443);
            let address = resolve_ip
                .parse()
                .map_err(|_| "WOU_ID_RESOLVE_IP is not a valid IP address".to_string())?;
            reqwest::Client::builder()
                .resolve(&host, std::net::SocketAddr::new(address, port))
                .build()
                .map_err(|error| format!("WOU-ID client build failed: {error}"))?
        } else {
            reqwest::Client::new()
        };
        // WOU-ID exposes the authenticated account through this existing
        // read-only route. It validates the bearer with WOU-ID's own JWT
        // middleware and returns the canonical account id without sharing the
        // signing secret with Shadows of War.
        let res = client
            .get(format!(
                "{}/api/v1/inventory/me",
                wou_url.trim_end_matches('/')
            ))
            .header("Authorization", format!("Bearer {token}"))
            .timeout(std::time::Duration::from_secs(3))
            .send()
            .await
            .map_err(|e| format!("WOU-ID service unreachable: {e}"))?;

        if !res.status().is_success() {
            return Err(format!("WOU-ID token rejected: HTTP {}", res.status()));
        }

        let resp: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Invalid WOU-ID JSON response: {e}"))?;
        let user_id = resp
            .get("account_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "WOU-ID response missing account_id".to_string())?;

        return Ok(user_id.to_string());
    }
    if provider == "crazygames" {
        let Some(token) = auth_token else {
            return Err("CrazyGames requests require X-Platform-Auth token".into());
        };
        let verified_id = crazygames::verify_user_token(token).await?;
        if !external_id.is_empty() && external_id != verified_id {
            warn!("CrazyGames external_id mismatch: client={external_id} token={verified_id}");
        }
        return Ok(verified_id);
    }
    Err("unsupported provider; anonymous clients must use /profile/anonymous".into())
}

/// Verify if the request has the correct Authorization bearer secret
fn verify_internal_auth(headers: &HeaderMap, secret_token: &str) -> bool {
    if let Some(auth_header) = headers.get(header::AUTHORIZATION)
        && let Ok(auth_str) = auth_header.to_str()
        && let Some(token) = auth_str.strip_prefix("Bearer ")
    {
        return token.trim() == secret_token;
    }
    false
}

/// GET /profile handler (platform token verification for signed-in providers)
async fn handle_get_profile(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<ProfileQuery>,
) -> impl IntoResponse {
    let request_id = identity_request_hint(&headers);
    let provider = query.provider.trim();
    let external_id = query.external_id.trim();
    let auth_token = platform_auth_token(&headers);

    info!(
        "[identity] platform profile request id={request_id} provider={provider} external_id_len={}",
        external_id.chars().count()
    );

    if provider.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "provider cannot be empty".to_string(),
            }),
        )
            .into_response();
    }

    let resolved_external_id = match if provider == "playgames" {
        state
            .verify_playgames_session(Some(external_id), auth_token.as_deref().unwrap_or(""))
            .map(|identity| identity.external_id)
            .map_err(|error| error.to_string())
    } else {
        resolve_external_id(provider, external_id, auth_token.as_deref()).await
    } {
        Ok(id) => id,
        Err(e) => {
            warn!("[identity] platform profile failed id={request_id} provider={provider}: {e}");
            return (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error: e })).into_response();
        }
    };

    let account_provider = if provider == "playgames" {
        "playgames_android"
    } else {
        provider
    };
    match state
        .db
        .get_or_create(account_provider.to_string(), resolved_external_id)
        .await
    {
        Ok(account) => {
            info!(
                "[identity] platform profile ack id={request_id} account={} name_len={}",
                account_hint(Some(&account.id)),
                account.display_name.chars().count()
            );
            (StatusCode::OK, Json(account.without_auth_secret())).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// POST /match/start (internal matchmaking registration)
async fn handle_match_start(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<MatchStartRequest>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        warn!("Unauthorized access attempt to /match/start");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }

    match state
        .db
        .register_match_start(&payload.match_id, &payload.player_ids)
        .await
    {
        Ok(()) => {
            let humans = state.db.count_human_players(&payload.player_ids).await;
            emit_event_line(
                &state,
                "match_started",
                None,
                serde_json::json!({
                    "match_id": payload.match_id,
                    "players": payload.player_ids.len(),
                    "humans": humans,
                }),
            );
            if let Err(e) = state.db.record_product_event("match_started", None).await {
                warn!("match_started analytics counter failed: {e}");
            }
            (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// POST /internal/match-finalize (relay triggers after authoritative exit logging)
async fn handle_match_finalize(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<MatchFinalizeRequest>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        warn!("Unauthorized access attempt to /internal/match-finalize");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }

    // Commit the replay before finalizing statistics. The bounded request and
    // replay sizes prevent a malformed match from becoming an unbounded
    // allocation, while fsync+rename prevents a stats ACK from preceding a
    // durable replay.
    if let Some(ref replay_bytes) = payload.replay_data {
        if replay_bytes.len() > MAX_REPLAY_BYTES {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(ErrorResponse {
                    error: format!("replay exceeds {} byte limit", MAX_REPLAY_BYTES),
                }),
            )
                .into_response();
        }
        let replay_dir = std::env::var("SOW_REPLAY_DIR").unwrap_or_else(|_| "replays".to_string());
        let file_path =
            std::path::Path::new(&replay_dir).join(format!("{}.replay", payload.match_id));

        if let Some(parent) = file_path.parent()
            && let Err(e) = tokio::fs::create_dir_all(parent).await
        {
            error!("Failed to create replay directory {:?}: {}", parent, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "replay directory unavailable".to_string(),
                }),
            )
                .into_response();
        }

        let temp_path = file_path.with_extension("replay.tmp");
        let write_result = async {
            let mut file = tokio::fs::File::create(&temp_path).await?;
            file.write_all(replay_bytes).await?;
            file.sync_all().await?;
            drop(file);
            tokio::fs::rename(&temp_path, &file_path).await
        }
        .await;
        match write_result {
            Ok(()) => {
                info!(
                    "Successfully committed raw replay for match {} to durable storage: {:?}",
                    payload.match_id, file_path
                );
            }
            Err(e) => {
                error!(
                    "Failed to commit raw replay file for match {} to disk: {}",
                    payload.match_id, e
                );
                let _ = tokio::fs::remove_file(&temp_path).await;
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "replay storage unavailable".to_string(),
                    }),
                )
                    .into_response();
            }
        }
    }

    // ponytail: Write lobby metadata JSON if provided alongside the replay
    if let Some(ref lobby_json) = payload.lobby_json {
        let replay_dir = std::env::var("SOW_REPLAY_DIR").unwrap_or_else(|_| "replays".to_string());
        let meta_path =
            std::path::Path::new(&replay_dir).join(format!("{}.json", payload.match_id));

        if let Some(parent) = meta_path.parent()
            && let Err(e) = tokio::fs::create_dir_all(parent).await
        {
            error!("Failed to create metadata directory {:?}: {}", parent, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "metadata directory unavailable".to_string(),
                }),
            )
                .into_response();
        }

        let temp_path = meta_path.with_extension("json.tmp");
        let write_result = async {
            let mut file = tokio::fs::File::create(&temp_path).await?;
            file.write_all(lobby_json.as_bytes()).await?;
            file.sync_all().await?;
            drop(file);
            tokio::fs::rename(&temp_path, &meta_path).await
        }
        .await;
        if let Err(e) = write_result {
            error!(
                "Failed to commit metadata JSON for match {} to disk: {}",
                payload.match_id, e
            );
            let _ = tokio::fs::remove_file(&temp_path).await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "metadata storage unavailable".to_string(),
                }),
            )
                .into_response();
        }
    }

    // Capture the roster/exit order before finalize deletes the Valkey keys,
    // so the analytics sink can record an aggregate match_ended event.
    let (participants, exits) = match state.db.match_participants(&payload.match_id).await {
        Ok(value) => value,
        Err(error) => {
            error!(
                "match {} participant snapshot failed before finalize: {error}",
                payload.match_id
            );
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "match participant state unavailable".to_string(),
                }),
            )
                .into_response();
        }
    };

    match state
        .db
        .finalize_match_with_lobby(&payload.match_id, payload.lobby_json.as_deref())
        .await
    {
        Ok(playgames_outcomes) => {
            if !participants.is_empty() {
                let humans = state.db.count_human_players(&participants).await;
                let winner = participants
                    .iter()
                    .find(|p| !exits.contains(p))
                    .cloned()
                    .or_else(|| exits.last().cloned());
                emit_event_line(
                    &state,
                    "match_ended",
                    None,
                    serde_json::json!({
                        "match_id": payload.match_id,
                        "players": participants.len(),
                        "humans": humans,
                        "winner_account_id": winner,
                    }),
                );
                if let Err(e) = state.db.record_product_event("match_ended", None).await {
                    warn!("match_ended analytics counter failed: {e}");
                }
                if let Err(e) = state.db.record_match_activation(&participants).await {
                    warn!("match activation analytics failed: {e}");
                }
                if let Err(error) = state.sync_playgames_match_event(&participants).await {
                    error!("Play Games match event sync failed: {error}");
                }
                for outcome in playgames_outcomes {
                    if let Err(error) = state.sync_playgames_match_outcome(&outcome).await {
                        error!(
                            "Play Games match outcome sync failed for account={}: {error}",
                            account_hint(Some(&outcome.account_id))
                        );
                    }
                }
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({ "status": "finalized" })),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to finalize match {}: {}", payload.match_id, e);
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// POST /internal/bot-pool/seed — resolve or create persistent bot accounts
/// for the given external_ids (provider="bot"). Idempotent: re-calling with
/// the same external_ids returns the same account_ids in the same order.
/// The display_name mapping is held by the caller; this endpoint only
/// guarantees stable (external_id → account_id) linkage.
async fn handle_bot_pool_seed(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<BotPoolSeedRequest>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        warn!("Unauthorized access attempt to /internal/bot-pool/seed");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }

    match state.db.seed_bot_pool(payload.external_ids).await {
        Ok(account_ids) => {
            (StatusCode::OK, Json(BotPoolSeedResponse { account_ids })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// POST /internal/save handler (authenticated server-to-server only)
async fn handle_direct_save(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<DirectSaveRequest>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        warn!("Unauthorized access attempt to /internal/save");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
            }),
        )
            .into_response();
    }

    match state
        .db
        .update_profile(&payload.account_id, payload.profile)
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account)).into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn handle_internal_stats(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let db_path = std::path::Path::new(&state.redb_path);
    let file_size = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);

    Json(serde_json::json!({
        "redb": {
            "path": state.redb_path,
            "file_size_bytes": file_size,
        }
    }))
}
