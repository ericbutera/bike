//! Bike sessions issued by local development login or production SSO.

pub mod oauth;
pub mod oauth_provider_service;
pub mod provider_settings;

use crate::auth::entities::{auth_events, refresh_tokens, users};
use crate::auth::error::AuthError;
use crate::auth::tokens::{generate_access_token, generate_refresh_token};
use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct MessageResponse {
    pub message: String,
}

#[derive(Serialize, Debug, Clone, ToSchema)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub pid: String,
    pub name: String,
    pub email: String,
    pub is_admin: bool,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct UserResponse {
    pub pid: String,
    pub name: String,
    pub email: String,
    pub is_admin: bool,
    pub disabled: bool,
    pub verified: bool,
}

impl From<users::Model> for UserResponse {
    fn from(user: users::Model) -> Self {
        Self {
            pid: user.pid.to_string(),
            name: user.name,
            email: user.email,
            is_admin: user.is_admin.unwrap_or(false),
            disabled: user.disabled,
            verified: user.email_verified_at.is_some(),
        }
    }
}

#[derive(Clone)]
pub struct SessionService {
    db: DatabaseConnection,
    jwt_secret: String,
}

impl SessionService {
    pub fn new(db: DatabaseConnection, jwt_secret: String) -> Self {
        Self { db, jwt_secret }
    }

    pub async fn refresh(
        &self,
        db: &DatabaseConnection,
        token: String,
    ) -> Result<TokenResponse, AuthError> {
        let stored = refresh_tokens::Model::find_by_token(db, &token)
            .await?
            .ok_or_else(|| AuthError::unauthorized("Invalid refresh token"))?;

        if stored.expires_at <= Utc::now().timestamp() {
            let _ = refresh_tokens::Model::remove_by_token(db, &stored.token).await;
            self.record_event(
                auth_events::EventType::TokenRefreshFailed,
                None,
                Some("expired"),
            )
            .await;
            return Err(AuthError::unauthorized("Token expired"));
        }

        let user = users::Model::find_by_pid(db, &stored.user_pid)
            .await?
            .ok_or_else(|| AuthError::unauthorized("User not found"))?;
        if user.disabled {
            return Err(AuthError::unauthorized("Invalid refresh token"));
        }
        if !refresh_tokens::Model::consume(db, &stored.token).await? {
            return Err(AuthError::unauthorized("Invalid refresh token"));
        }
        let response = self.issue_tokens(db, &user).await?;
        self.record_event(auth_events::EventType::TokenRefresh, Some(&user), None)
            .await;
        crate::platform::api_metrics::token_refresh_counter().inc();
        Ok(response)
    }

    pub async fn logout(&self, _pid: Uuid) -> Result<(), AuthError> {
        self.record_event(auth_events::EventType::Logout, None, None)
            .await;
        crate::platform::api_metrics::logout_counter().inc();
        Ok(())
    }

    pub async fn issue_tokens(
        &self,
        db: &DatabaseConnection,
        user: &users::Model,
    ) -> Result<TokenResponse, AuthError> {
        user.ensure_enabled()?;
        let access_token = generate_access_token(user, &self.jwt_secret)?;
        let (refresh_token, _) = generate_refresh_token();
        refresh_tokens::Model::create_record(db, user.pid, &refresh_token).await?;
        Ok(TokenResponse {
            access_token,
            refresh_token,
            pid: user.pid.to_string(),
            name: user.name.clone(),
            email: user.email.clone(),
            is_admin: user.is_admin.unwrap_or_default(),
        })
    }

    async fn record_event(
        &self,
        event: auth_events::EventType,
        user: Option<&users::Model>,
        reason: Option<&str>,
    ) {
        let payload = auth_events::AuthEventPayload {
            user_id: user.map(|user| user.id),
            email: user.map(|user| user.email.clone()),
            reason: reason.map(str::to_owned),
            ..Default::default()
        };
        let _ = auth_events::Model::record(&self.db, &event, payload).await;
    }
}
