use crate::auth::entities::{api_clients, users};
use crate::auth::error::AuthError;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    User,
    ApiClient,
}

pub const ACCESS_TOKEN_TTL_MINUTES: i64 = 30;

pub fn access_token_ttl_seconds() -> i64 {
    ACCESS_TOKEN_TTL_MINUTES * 60
}

fn default_token_type() -> TokenType {
    TokenType::User
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    #[serde(default = "default_token_type")]
    pub token_type: TokenType,
}

/// Configuration trait for JWT secret
pub trait JwtConfig {
    fn jwt_secret(&self) -> String;
}

pub fn generate_access_token(
    user: &users::Model,
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    let exp = Utc::now()
        .checked_add_signed(Duration::minutes(ACCESS_TOKEN_TTL_MINUTES))
        .unwrap()
        .timestamp();
    let claims = Claims {
        sub: user.pid.to_string(),
        exp,
        token_type: TokenType::User,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )?;
    Ok(token)
}

pub fn generate_api_client_access_token(
    client: &api_clients::Model,
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    let exp = Utc::now()
        .checked_add_signed(Duration::minutes(ACCESS_TOKEN_TTL_MINUTES))
        .unwrap()
        .timestamp();
    let claims = Claims {
        sub: client.client_id.to_string(),
        exp,
        token_type: TokenType::ApiClient,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
}

pub fn generate_refresh_token() -> (String, i64) {
    let exp = Utc::now()
        .checked_add_signed(Duration::days(7))
        .unwrap()
        .timestamp();
    let token = Uuid::new_v4().to_string();
    (token, exp)
}

pub fn verify_access_token(token: &str, secret: &str) -> Result<TokenData<Claims>, AuthError> {
    let validation = Validation::default();
    let td = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &validation,
    )
    .map_err(|_| AuthError::unauthorized("Invalid token"))?;

    // Enforce expiry explicitly
    let now = Utc::now().timestamp();
    if td.claims.exp < now {
        return Err(AuthError::unauthorized("Token expired"));
    }

    Ok(td)
}
