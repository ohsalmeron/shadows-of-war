use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;

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
