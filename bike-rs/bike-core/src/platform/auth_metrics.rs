use crate::auth::entities::{auth_events, refresh_tokens, users};
use chrono::{DateTime, Duration, Utc};
use sea_orm::{DatabaseConnection, DbErr};

/// Retained authentication records at scrape time, including legacy auth events.
#[derive(Debug)]
pub struct AuthMetrics {
    pub failed_logins_last_30d: u64,
    pub password_resets_last_30d: u64,
    pub refresh_tokens_created_last_30d: u64,
    pub new_signups_last_24h: u64,
}

impl AuthMetrics {
    pub async fn fetch(db: &DatabaseConnection, now: DateTime<Utc>) -> Result<Self, DbErr> {
        let since = now - Duration::days(30);
        let (
            failed_logins_last_30d,
            password_resets_last_30d,
            refresh_tokens_created_last_30d,
            new_signups_last_24h,
        ) = tokio::try_join!(
            auth_events::Model::count_event_type_since(
                db,
                &auth_events::EventType::LoginFailed,
                since
            ),
            auth_events::Model::count_event_type_since(
                db,
                &auth_events::EventType::PasswordResetRequest,
                since
            ),
            refresh_tokens::Model::count_created_since(db, since),
            users::Model::count_created_since(db, now - Duration::days(1)),
        )?;
        Ok(Self {
            failed_logins_last_30d,
            password_resets_last_30d,
            refresh_tokens_created_last_30d,
            new_signups_last_24h,
        })
    }
}
