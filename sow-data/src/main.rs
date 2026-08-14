use sow_data::crazygames;
use sow_data::db::{PlayerDb, PlayerProfile};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Query, State},
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
    redb_path: String,
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
}

#[derive(Deserialize)]
struct AnonymousDisplayNameRequest {
    account_id: String,
    display_name: String,
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
        "Config - Port: {}, Valkey: {}, Secret: [REDACTED], CG API Key Configured: {}",
        port,
        sanitized_valkey,
        crazygames_api_key.is_some()
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

    let state = Arc::new(AppState {
        db: player_db,
        secret_token,
        redb_path: redb_path.clone(),
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
        .route("/profile", get(handle_get_profile))
        .route("/profile/anonymous", post(handle_anonymous_profile))
        .route(
            "/profile/anonymous/name",
            post(handle_anonymous_display_name),
        )
        .route("/match/start", post(handle_match_start))
        .route("/internal/match-finalize", post(handle_match_finalize))
        .route("/internal/save", post(handle_direct_save))
        .route("/internal/stats", get(handle_internal_stats))
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

/// Liveness probe for the pipeline and service supervisor. It deliberately
/// performs no profile lookup and emits no authentication warning.
async fn handle_healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "ok": true })))
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
        )
        .await
    {
        Ok(account) => {
            info!(
                "[identity] profile ack id={request_id} account={} name_len={}",
                account_hint(Some(&account.id)),
                account.display_name.chars().count()
            );
            (StatusCode::OK, Json(account)).into_response()
        }
        Err(error) => {
            let message = error.to_string();
            let status = if message.contains("account_id must") {
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
        .update_anonymous_display_name(&payload.account_id, &payload.display_name)
        .await
    {
        Ok(account) => {
            info!(
                "[identity] rename ack id={request_id} account={} name_len={}",
                account_hint(Some(&account.id)),
                account.display_name.chars().count()
            );
            (StatusCode::OK, Json(account)).into_response()
        }
        Err(error) => {
            let message = error.to_string();
            let status = if message.contains("account_id must") || message.contains("display_name")
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
            (StatusCode::OK, Json(account)).into_response()
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
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response(),
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

    match state.db.finalize_match(&payload.match_id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "finalized" })),
        )
            .into_response(),
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
