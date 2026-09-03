use sow_data::crazygames;
use sow_data::db::{PlayerDb, PlayerProfile};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
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
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse { error }),
        )
            .into_response();
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
                Json(ErrorResponse { error: "profile not found".to_string() }),
            )
                .into_response();
        }
        Err(error) => {
            error!("skin unlock profile lookup failed: {error}");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse { error: "profile unavailable".to_string() }),
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
    match state.db.unlock_skin_with_gems(&account_id, &payload.skin_id).await {
        Ok(account) => (StatusCode::OK, Json(account.without_auth_secret())).into_response(),
        Err(error) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse { error: error.to_string() }),
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
                Json(ErrorResponse { error: "profile not found".to_string() }),
            )
                .into_response();
        }
        Err(error) => {
            error!("skin equip profile lookup failed: {error}");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse { error: "profile unavailable".to_string() }),
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
            Json(ErrorResponse { error: error.to_string() }),
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
    if event.event_type != "NON_RENEWING_PURCHASE" {
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
        Ok((_account, false)) => (
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
        "wou" | "wou_id" | "world_of_unreal" => {
            match resolve_external_id("wou", "", Some(&payload.token)).await {
                Ok(external_id) => state
                    .db
                    .get_or_create("wou".to_string(), external_id)
                    .await
                    .map(|account| account.id)
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e),
            }
        }
        "crazygames" => match resolve_external_id("crazygames", "", Some(&payload.token)).await {
            Ok(external_id) => state
                .db
                .get_or_create("crazygames".to_string(), external_id)
                .await
                .map(|account| account.id)
                .map_err(|e| e.to_string()),
            Err(e) => Err(e),
        },
        "anonymous" => match (payload.account_id.as_deref(), payload.token.as_str()) {
            (Some(account_id), token) if !account_id.trim().is_empty() => {
                state.db.verify_anonymous_secret(account_id, token).await
            }
            _ => Err("anonymous verification requires account_id and token".to_string()),
        },
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
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse { error: e }),
            )
                .into_response()
        }
    }
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
        .route("/match/start", post(handle_match_start))
        .route("/internal/match-finalize", post(handle_match_finalize))
        .route("/internal/save", post(handle_direct_save))
        .route("/internal/stats", get(handle_internal_stats))
        .route("/internal/verify", post(handle_internal_verify))
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
    match state.db.search_public_profiles(query_text, cursor.saturating_add(limit)).await {
        Ok(mut profiles) => {
            if cursor < profiles.len() {
                profiles.drain(..cursor);
            } else {
                profiles.clear();
            }
            profiles.truncate(limit);
            let has_next = profiles.len() == limit;
            let next_cursor = has_next.then_some(cursor.saturating_add(limit));
            (StatusCode::OK, Json(serde_json::json!({
                "items": profiles,
                "next_cursor": next_cursor
            }))).into_response()
        }
        Err(error) => {
            error!("public profile search failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse { error: "profiles unavailable".to_string() }),
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
            Json(ErrorResponse { error: "profile not found".to_string() }),
        )
            .into_response(),
        Err(error) => {
            error!("public profile lookup failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse { error: "profiles unavailable".to_string() }),
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
            (StatusCode::OK, Json(serde_json::json!({
                "items": items,
                "next_cursor": next_cursor,
            })))
                .into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: "profile not found".to_string() }),
        )
            .into_response(),
        Err(error) => {
            error!("public match history failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse { error: "match history unavailable".to_string() }),
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
            Json(ErrorResponse { error: "match not found".to_string() }),
        )
            .into_response(),
        Err(error) => {
            error!("public match detail failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse { error: "match unavailable".to_string() }),
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
        Ok(Some(ratings)) => (StatusCode::OK, Json(serde_json::json!({ "items": ratings }))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse { error: "profile not found".to_string() }),
        )
            .into_response(),
        Err(error) => {
            error!("public profile seasons failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse { error: "seasons unavailable".to_string() }),
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
            (StatusCode::OK, Json(serde_json::json!({
                "season": current,
                "items": seasons,
            })))
                .into_response()
        }
        Err(error) => {
            error!("current season lookup failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse { error: "seasons unavailable".to_string() }),
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
            Json(ErrorResponse { error: "season not found".to_string() }),
        )
            .into_response();
    }
    let queue = query.queue.as_deref().unwrap_or("Matchmaking");
    let mode = query.mode.as_deref().unwrap_or("FFA");
    let limit = query.limit.unwrap_or(100).clamp(1, 100);
    let cursor = query.cursor.unwrap_or(0).min(10_000);
    match state.db.public_leaderboard(queue, mode, cursor.saturating_add(limit)).await {
        Ok(mut items) => {
            if cursor < items.len() {
                items.drain(..cursor);
            } else {
                items.clear();
            }
            items.truncate(limit);
            let has_next = items.len() == limit;
            let next_cursor = has_next.then_some(cursor.saturating_add(limit));
            (StatusCode::OK, Json(serde_json::json!({
                "season_id": season_id,
                "queue": queue,
                "mode": mode,
                "items": items,
                "next_cursor": next_cursor,
            })))
                .into_response()
        }
        Err(error) => {
            error!("public leaderboard lookup failed: {error}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse { error: "leaderboard unavailable".to_string() }),
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
                    warn!("[identity] secret mint failed for {}: {e}", account_hint(Some(&account.id)));
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
            } else if message.contains("account_id must") || message.contains("display_name")
            {
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
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse { error }),
            )
                .into_response();
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
        let wou_url = std::env::var("WOU_ID_URL").unwrap_or_else(|_| "http://127.0.0.1:25570".into());
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
            .get(format!("{}/api/v1/inventory/me", wou_url.trim_end_matches('/')))
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

    let resolved_external_id =
        match resolve_external_id(provider, external_id, auth_token.as_deref()).await {
            Ok(id) => id,
            Err(e) => {
                warn!(
                    "[identity] platform profile failed id={request_id} provider={provider}: {e}"
                );
                return (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error: e }))
                    .into_response();
            }
        };

    match state
        .db
        .get_or_create(provider.to_string(), resolved_external_id)
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
        Ok(()) => {
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
