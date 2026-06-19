use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct CrazyTokenClaims {
    #[serde(rename = "userId")]
    user_id: String,
}

#[derive(Deserialize)]
struct PublicKeyResponse {
    #[serde(rename = "publicKey")]
    public_key: String,
}

#[derive(Serialize)]
struct ScoreEntry {
    #[serde(rename = "userId")]
    user_id: String,
    score: u32,
    timestamp: String,
}

#[derive(Serialize)]
struct SubmitScoresRequest {
    scores: Vec<ScoreEntry>,
}

pub async fn verify_user_token(token: &str) -> Result<String, String> {
    let resp = reqwest::get("https://sdk.crazygames.com/publicKey.json")
        .await
        .map_err(|e| format!("failed to fetch CrazyGames public key: {e}"))?;
    let key_body: PublicKeyResponse = resp
        .json()
        .await
        .map_err(|e| format!("invalid CrazyGames public key response: {e}"))?;
    if key_body.public_key.is_empty() {
        return Err("CrazyGames public key is empty".into());
    }

    let key = DecodingKey::from_rsa_pem(key_body.public_key.as_bytes())
        .map_err(|e| format!("invalid CrazyGames public key PEM: {e}"))?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = true;

    let token_data = decode::<CrazyTokenClaims>(token, &key, &validation)
        .map_err(|e| format!("CrazyGames token verification failed: {e}"))?;
    if token_data.claims.user_id.is_empty() {
        return Err("CrazyGames token missing userId".into());
    }
    Ok(token_data.claims.user_id)
}

pub fn iso_8601_timestamp() -> String {
    let dt = crate::time_util::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second, dt.millisecond
    )
}

pub async fn submit_score(api_key: &str, user_id: &str, score: u32) -> Result<(), String> {
    let timestamp = iso_8601_timestamp();
    let payload = SubmitScoresRequest {
        scores: vec![ScoreEntry {
            user_id: user_id.to_string(),
            score,
            timestamp,
        }],
    };

    let client = reqwest::Client::new();
    let resp = client
        .post("https://leaderboard.crazygames.com/leaderboard/scores")
        .header("X-API-Key", api_key)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Network request failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!(
            "Leaderboard score submission failed with status {status}: {text}"
        ));
    }

    log::info!("Successfully submitted score {score} for CrazyGames user {user_id}");
    Ok(())
}
