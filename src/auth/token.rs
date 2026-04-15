use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

const SECRET: &[u8] = b"chat_secret_key";

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub username: String,
    pub exp: usize,
}

pub fn sign_token(username: &str) -> anyhow::Result<String> {
    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as usize
        + 7 * 24 * 3600; // 7 天

    let claims = Claims {
        username: username.to_string(),
        exp,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(SECRET))
        .map_err(Into::into)
}

pub fn verify_token(token: &str) -> anyhow::Result<String> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(SECRET),
        &Validation::new(Algorithm::HS256),
    )?;
    Ok(data.claims.username)
}