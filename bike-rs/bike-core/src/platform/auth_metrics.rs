use crate::auth::entities::{auth_events, refresh_tokens, users};
use crate::platform::aggregator::{Aggregator, NamedStat, StatResult};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use utoipa::ToSchema;

/// Bike session metrics, including historical password-auth events.
/// Collect these via `AuthMetrics::fetch(db)` and include them in your
/// admin aggregates response.
#[derive(Serialize, ToSchema, Debug)]
pub struct AuthMetrics {
    pub failed_logins_last_30d: StatResult,
    pub password_resets_last_30d: StatResult,
    pub refresh_tokens_issued_last_30d: StatResult,
    pub new_signups_last_24h: StatResult,
}

impl AuthMetrics {
    /// Concurrently fetches all auth metrics from the database.
    pub async fn fetch(db: &DatabaseConnection) -> Self {
        let f_failed_logins = auth_events::Model::count_failed_logins_last_30d(db);
        let f_password_resets = auth_events::Model::count_password_resets_last_30d(db);
        let f_tokens = Aggregator::recent_count::<refresh_tokens::Entity>(
            db,
            refresh_tokens::Column::CreatedAt,
            30,
        );
        let f_signups = Aggregator::recent_count::<users::Entity>(db, users::Column::CreatedAt, 1);

        let (failed_logins_res, password_resets_res, tokens_res, signups_res) =
            tokio::join!(f_failed_logins, f_password_resets, f_tokens, f_signups);

        AuthMetrics {
            failed_logins_last_30d: StatResult::from_result(failed_logins_res),
            password_resets_last_30d: StatResult::from_result(password_resets_res),
            refresh_tokens_issued_last_30d: StatResult::from_result(tokens_res),
            new_signups_last_24h: StatResult::from_result(signups_res),
        }
    }

    /// Converts to a `Vec<NamedStat>` for use in sectioned metrics responses.
    pub fn into_named_stats(self) -> Vec<NamedStat> {
        vec![
            NamedStat {
                key: "failed_logins_last_30d".into(),
                label: "Failed Logins (30d)".into(),
                desc: "last 30 days".into(),
                value: self.failed_logins_last_30d.value,
                error: self.failed_logins_last_30d.error,
            },
            NamedStat {
                key: "password_resets_last_30d".into(),
                label: "Password Resets (30d)".into(),
                desc: "last 30 days".into(),
                value: self.password_resets_last_30d.value,
                error: self.password_resets_last_30d.error,
            },
            NamedStat {
                key: "refresh_tokens_issued_last_30d".into(),
                label: "Refresh Tokens Issued (30d)".into(),
                desc: "last 30 days".into(),
                value: self.refresh_tokens_issued_last_30d.value,
                error: self.refresh_tokens_issued_last_30d.error,
            },
            NamedStat {
                key: "new_signups_last_24h".into(),
                label: "New Signups (24h)".into(),
                desc: "last 24 hours".into(),
                value: self.new_signups_last_24h.value,
                error: self.new_signups_last_24h.error,
            },
        ]
    }
}
