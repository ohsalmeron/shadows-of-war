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
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs() as i64;
    let ms = duration.subsec_millis();

    let days = secs.div_euclid(86400);
    let rem_secs = secs.rem_euclid(86400);
    let hour = rem_secs / 3600;
    let min = (rem_secs % 3600) / 60;
    let sec = rem_secs % 60;

    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mp < 10 { y } else { y + 1 };

    format!("{y:04}-{m:02}-{d:02}T{hour:02}:{min:02}:{sec:02}.{ms:03}Z")
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
