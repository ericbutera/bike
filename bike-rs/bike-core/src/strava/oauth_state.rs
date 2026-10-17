//! Browser-bound, single-use Strava authorization state.
use crate::entities::strava_connections;
use crate::{auth::entities::oauth_states, workflow_error::WorkflowError};
use axum::http::HeaderMap;
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use sea_orm::DatabaseConnection;
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;
const PROVIDER: &str = "strava";
const COOKIE_NAME: &str = "strava_oauth_session";
const STATE_TTL_SECONDS: i64 = 600;

pub struct AuthorizationState {
    pub token: String,
    pub browser_session: String,
}

pub struct VerifiedStravaState {
    pub user_id: i32,
    pub issued_at: DateTime<Utc>,
    pub nonce: String,
}

#[derive(serde::Deserialize)]
pub struct AuthorizationCallback {
    pub code: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

impl AuthorizationCallback {
    pub fn is_incomplete(&self) -> bool {
        let missing_fields = self.code.is_none() || self.state.is_none();
        self.error.is_some() || missing_fields
    }
}

#[async_trait::async_trait]
pub trait AuthorizationExchange: Send + Sync {
    async fn exchange(
        &self,
        code: &str,
        state: VerifiedStravaState,
    ) -> Result<strava_connections::Model, WorkflowError>;
}

#[async_trait::async_trait]
impl AuthorizationExchange for DatabaseConnection {
    async fn exchange(
        &self,
        code: &str,
        state: VerifiedStravaState,
    ) -> Result<strava_connections::Model, WorkflowError> {
        super::exchange_code_for_connection(self, code, state).await
    }
}

#[async_trait::async_trait]
pub trait OAuthStateStore: Send + Sync {
    async fn save(&self, state: &AuthorizationState) -> Result<(), WorkflowError>;
    async fn consume(&self, token: &str) -> Result<(), WorkflowError>;
}

#[async_trait::async_trait]
impl OAuthStateStore for DatabaseConnection {
    async fn save(&self, state: &AuthorizationState) -> Result<(), WorkflowError> {
        oauth_states::Model::save(self, PROVIDER, &state.token, &state.browser_session)
            .await
            .map_err(Into::into)
    }

    async fn consume(&self, token: &str) -> Result<(), WorkflowError> {
        oauth_states::Model::consume(self, PROVIDER, token)
            .await
            .map(|_| ())
            .map_err(Into::into)
    }
}

pub struct OAuthState<'a> {
    secret: &'a str,
}

impl<'a> OAuthState<'a> {
    pub fn new(secret: &'a str) -> Self {
        Self { secret }
    }

    pub async fn callback(
        &self,
        store: &impl OAuthStateStore,
        exchange: &impl AuthorizationExchange,
        callback: &AuthorizationCallback,
        browser_session: &str,
        now: DateTime<Utc>,
    ) -> Result<strava_connections::Model, WorkflowError> {
        let token = callback.state.as_deref().ok_or_else(invalid_state)?;
        let verified = self.consume(store, token, browser_session, now).await?;
        if let Some(error) = callback.error.as_deref() {
            return Err(WorkflowError::bad_request(format!(
                "Strava authorization was not completed: {error}"
            )));
        }
        let code = callback
            .code
            .as_deref()
            .filter(|code| !code.is_empty())
            .ok_or_else(|| WorkflowError::bad_request("Missing Strava authorization code"))?;
        exchange.exchange(code, verified).await
    }

    pub async fn issue(
        &self,
        store: &impl OAuthStateStore,
        user_id: i32,
        now: DateTime<Utc>,
    ) -> Result<AuthorizationState, WorkflowError> {
        let browser_session = Uuid::new_v4().simple().to_string();
        let payload = format!("{user_id}:{}:{browser_session}", now.timestamp());
        let signature = hex::encode(self.mac(&payload)?.finalize().into_bytes());
        let state = AuthorizationState {
            token: format!("{payload}:{signature}"),
            browser_session,
        };
        store.save(&state).await?;
        Ok(state)
    }

    pub async fn consume(
        &self,
        store: &impl OAuthStateStore,
        token: &str,
        browser_session: &str,
        now: DateTime<Utc>,
    ) -> Result<VerifiedStravaState, WorkflowError> {
        let state = self.verify(token, browser_session, now)?;
        store.consume(token).await?;
        Ok(state)
    }

    fn verify(
        &self,
        token: &str,
        browser_session: &str,
        now: DateTime<Utc>,
    ) -> Result<VerifiedStravaState, WorkflowError> {
        let parts: Vec<_> = token.split(':').collect();
        let [user, timestamp, nonce, signature] = parts.as_slice() else {
            return Err(invalid_state());
        };
        let signature = hex::decode(signature).map_err(|_| invalid_state())?;
        let payload = format!("{user}:{timestamp}:{nonce}");
        self.verify_signature(&payload, &signature)?;
        let browser_payload = format!("{user}:{timestamp}:{browser_session}");
        self.verify_signature(&browser_payload, &signature)?;
        let user_id = user.parse::<i32>().map_err(|_| invalid_state())?;
        let timestamp = timestamp.parse::<i64>().map_err(|_| invalid_state())?;
        let issued_at = DateTime::from_timestamp(timestamp, 0).ok_or_else(invalid_state)?;
        if issued_at > now || now >= issued_at + Duration::seconds(STATE_TTL_SECONDS) {
            return Err(invalid_state());
        }
        if user_id <= 0 || Uuid::parse_str(nonce).is_err() {
            return Err(invalid_state());
        }
        Ok(VerifiedStravaState {
            user_id,
            issued_at,
            nonce: (*nonce).into(),
        })
    }

    fn mac(&self, payload: &str) -> Result<HmacSha256, WorkflowError> {
        let mut mac = HmacSha256::new_from_slice(self.secret.as_bytes())
            .map_err(|_| WorkflowError::internal("Invalid Strava state signing key"))?;
        mac.update(payload.as_bytes());
        Ok(mac)
    }

    fn verify_signature(&self, payload: &str, signature: &[u8]) -> Result<(), WorkflowError> {
        self.mac(payload)?.verify_slice(signature).map_err(|_| invalid_state())
    }
}

pub fn browser_session<'a>(headers: &'a HeaderMap, api_url: &str) -> Result<&'a str, WorkflowError> {
    crate::auth::cookies::cookie_value(headers, cookie_name(api_url))
        .filter(|value| !value.is_empty())
        .ok_or_else(invalid_state)
}

pub fn session_cookie(value: &str, api_url: &str) -> String {
    cookie(value, api_url, STATE_TTL_SECONDS)
}

pub fn clear_session_cookie(api_url: &str) -> String {
    cookie("", api_url, 0)
}

fn cookie(value: &str, api_url: &str, max_age: i64) -> String {
    let name = cookie_name(api_url);
    let secure = if api_url.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    format!("{name}={value}; HttpOnly; Path=/; SameSite=Lax; Max-Age={max_age}{secure}")
}

fn cookie_name(api_url: &str) -> &'static str {
    if api_url.starts_with("https://") {
        "__Host-strava_oauth_session"
    } else {
        COOKIE_NAME
    }
}

fn invalid_state() -> WorkflowError {
    WorkflowError::bad_request("Strava connect session is invalid or expired. Try again.")
}

#[cfg(test)]
mod tests;
