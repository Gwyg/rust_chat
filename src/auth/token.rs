use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

// 随便生成的
const SECRET: &[u8] = b"a3f8c2e1d4b7f9e2a1c3d5e7f9b2c4d6e8f0a2b4c6d8e0f2a4b6c8d0e2f4a6b8";

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub username: String,
    pub exp: usize,
}

pub fn sign_token(username: &str) -> anyhow::Result<String> {
    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as usize
        + 1800; 

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