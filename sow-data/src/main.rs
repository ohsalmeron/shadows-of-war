use sow_data::{crazygames, db};
use sow_data::db::{LinkOutcome, PlayerDb, PlayerProfile};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

struct AppState {
    db: PlayerDb,
    secret_token: String,
    redb_path: String,
    redb_db: Arc<redb::Database>,
}

#[derive(Deserialize)]
struct ProfileQuery {
    provider: String,
    external_id: String,
    fallback_name: Option<String>,
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
struct LinkRequest {
    account_id: String,
    provider: String,
    external_id: String,
}

#[derive(Deserialize)]
struct ProfileLinkRequest {
    account_id: String,
    target_provider: String,
    target_external_id: String,
}

#[derive(Deserialize)]
struct ProfileLinkResolveRequest {
    account_id: String,
    keep_account_id: String,
    target_provider: String,
    target_external_id: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
struct ProfileLinkResponse {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    account: Option<db::PlayerAccount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    existing_account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    existing: Option<db::AccountSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current: Option<db::AccountSummary>,
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
        .unwrap_or_else(|_| "sow_db_dev_secret_123_change_me_in_prod".to_string());

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
    let redb_path = std::env::var("SOW_REDB_PATH").unwrap_or_else(|_| "sow_metadata.redb".to_string());
    info!("Opening persistent REDB database at {}", redb_path);
    if let Some(dir) = std::path::Path::new(&redb_path).parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).expect("Failed to create REDB data directory");
        }
    }
    let redb_db = sow_data::init_database(&redb_path).expect("Failed to initialize REDB metadata database");
    let redb_db_arc = Arc::new(redb_db);

    // Seed Valkey RAM from REDB on boot
    info!("Seeding Valkey RAM cache from REDB...");
    if let Err(e) = sow_data::metadata_db::seed_valkey_from_redb(&redb_db_arc, &valkey_url) {
        error!("Failed to seed Valkey on startup: {e}");
    }

    // Initialize database connector
    let player_db = PlayerDb::new(&valkey_url, crazygames_api_key, Some(Arc::clone(&redb_db_arc)));

    let state = Arc::new(AppState {
        db: player_db,
        secret_token,
        redb_path: redb_path.clone(),
        redb_db: Arc::clone(&redb_db_arc),
    });

    // Configure CORS for web portal compatibility
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::HeaderName::from_static("x-platform-auth"),
        ]);

    // Define router
    let app = Router::new()
        .route("/profile", get(handle_get_profile))
        .route("/profile/link", post(handle_profile_link))
        .route("/profile/link/resolve", post(handle_profile_link_resolve))
        .route("/match/start", post(handle_match_start))
        .route("/internal/match-finalize", post(handle_match_finalize))
        .route("/internal/save", post(handle_direct_save))
        .route("/internal/link", post(handle_link_identity))
        .route("/internal/stats", get(handle_internal_stats))
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
    if external_id.is_empty() {
        return Err("external_id cannot be empty".into());
    }
    Ok(external_id.to_string())
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
    let provider = query.provider.trim();
    let external_id = query.external_id.trim();
    let fallback_name = query.fallback_name.unwrap_or_else(|| "Player".to_string());
    let auth_token = platform_auth_token(&headers);

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
                warn!("Platform auth failed for /profile {provider}: {e}");
                return (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error: e }))
                    .into_response();
            }
        };

    match state
        .db
        .get_or_create(provider.to_string(), resolved_external_id, fallback_name)
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account)).into_response(),
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

    // ponytail: High-performance streaming of raw replay bytes directly to ZFS disk storage
    if let Some(ref replay_bytes) = payload.replay_data {
        let replay_dir = std::env::var("SOW_REPLAY_DIR").unwrap_or_else(|_| "replays".to_string());
        let file_path = std::path::Path::new(&replay_dir).join(format!("{}.replay", payload.match_id));
        
        if let Some(parent) = file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        
        match std::fs::write(&file_path, replay_bytes) {
            Ok(()) => {
                info!("Successfully wrote raw replay for match {} directly to ZFS disk storage: {:?}", payload.match_id, file_path);
            }
            Err(e) => {
                error!("Failed to write raw replay file for match {} to disk: {}", payload.match_id, e);
            }
        }
    }

    // ponytail: Write lobby metadata JSON if provided alongside the replay
    if let Some(ref lobby_json) = payload.lobby_json {
        let replay_dir = std::env::var("SOW_REPLAY_DIR").unwrap_or_else(|_| "replays".to_string());
        let meta_path = std::path::Path::new(&replay_dir).join(format!("{}.json", payload.match_id));
        
        if let Some(parent) = meta_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        
        if let Err(e) = std::fs::write(&meta_path, lobby_json) {
            error!("Failed to write metadata JSON file for match {} to disk: {}", payload.match_id, e);
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

/// POST /profile/link — attach a platform identity to the active account.
async fn handle_profile_link(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ProfileLinkRequest>,
) -> impl IntoResponse {
    let target_provider = payload.target_provider.trim();
    let target_external_id = payload.target_external_id.trim();
    let auth_token = platform_auth_token(&headers);
    let resolved_external_id =
        match resolve_external_id(target_provider, target_external_id, auth_token.as_deref()).await
        {
            Ok(id) => id,
            Err(e) => {
                warn!("Platform auth failed for /profile/link: {e}");
                return (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error: e }))
                    .into_response();
            }
        };

    match state
        .db
        .try_link_identity(
            &payload.account_id,
            target_provider.to_string(),
            resolved_external_id,
        )
        .await
    {
        Ok(LinkOutcome::Linked(account)) => {
            state.db.submit_crazygames_score(&account).await;
            (
                StatusCode::OK,
                Json(ProfileLinkResponse {
                    status: "linked".into(),
                    account: Some(account),
                    existing_account_id: None,
                    existing: None,
                    current: None,
                }),
            )
                .into_response()
        }
        Ok(LinkOutcome::Conflict {
            existing_account_id,
            existing,
            current,
        }) => (
            StatusCode::OK,
            Json(ProfileLinkResponse {
                status: "conflict".into(),
                account: None,
                existing_account_id: Some(existing_account_id),
                existing: Some(existing),
                current: Some(current),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// POST /profile/link/resolve — pick which account keeps progress after a conflict.
async fn handle_profile_link_resolve(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ProfileLinkResolveRequest>,
) -> impl IntoResponse {
    let target_provider = payload.target_provider.trim();
    let target_external_id = payload.target_external_id.trim();
    let auth_token = platform_auth_token(&headers);
    let resolved_external_id =
        match resolve_external_id(target_provider, target_external_id, auth_token.as_deref()).await
        {
            Ok(id) => id,
            Err(e) => {
                warn!("Platform auth failed for /profile/link/resolve: {e}");
                return (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error: e }))
                    .into_response();
            }
        };

    match state
        .db
        .resolve_link_conflict(
            &payload.account_id,
            &payload.keep_account_id,
            target_provider.to_string(),
            resolved_external_id,
        )
        .await
    {
        Ok(account) => {
            state.db.submit_crazygames_score(&account).await;
            (
                StatusCode::OK,
                Json(ProfileLinkResponse {
                    status: "resolved".into(),
                    account: Some(account),
                    existing_account_id: None,
                    existing: None,
                    current: None,
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// POST /internal/link handler (authenticated server-to-server only)
async fn handle_link_identity(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<LinkRequest>,
) -> impl IntoResponse {
    if !verify_internal_auth(&headers, &state.secret_token) {
        warn!("Unauthorized access attempt to /internal/link");
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
        .link_identity(&payload.account_id, payload.provider, payload.external_id)
        .await
    {
        Ok(account) => (StatusCode::OK, Json(account)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn handle_internal_stats(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let db_path = std::path::Path::new(&state.redb_path);
    let file_size = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);

    Json(serde_json::json!({
        "redb": {
            "path": state.redb_path,
            "file_size_bytes": file_size,
        }
    }))
}
