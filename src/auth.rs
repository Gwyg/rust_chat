use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

const SECRET: &[u8] = b"chat_secret_key"; // 后续可改成配置项

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub username: String,
    pub exp: usize,
}

/// 签发 token
pub fn sign_token(username: &str) -> anyhow::Result<String> {
    let claims = Claims {
        username: username.to_string(),
        exp: 9999999999, // 暂时不过期
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(SECRET),
    )?;
    Ok(token)
}

/// 验证 token，返回 username
pub fn verify_token(token: &str) -> anyhow::Result<String> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(SECRET),
        &Validation::new(Algorithm::HS256),
    )?;
    Ok(data.claims.username)
}