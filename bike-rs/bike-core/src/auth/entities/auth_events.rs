use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{FromQueryResult, NotSet, QuerySelect, Set};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "auth_events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub ts: DateTime<Utc>,
    pub event_type: String,
    pub user_id: Option<i32>,
    pub email: Option<String>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub reason: Option<String>,
    pub meta: Option<serde_json::Value>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(FromQueryResult)]
struct LastLoginRow {
    last_login_at: Option<DateTime<Utc>>,
}

impl Model {
    pub async fn last_login(db: &DatabaseConnection, user_id: i32) -> Result<Option<DateTime<Utc>>, DbErr> {
        let row = Entity::find()
            .select_only()
            .column_as(Column::Ts.max(), "last_login_at")
            .filter(Column::EventType.eq(EventType::LoginSucceeded.to_string()))
            .filter(Column::UserId.eq(user_id))
            .into_model::<LastLoginRow>()
            .one(db)
            .await?;
        Ok(row.and_then(|row| row.last_login_at))
    }

    pub async fn for_user_page(
        db: &DatabaseConnection,
        user_id: i32,
        page: u64,
        per_page: u64,
    ) -> Result<(Vec<Self>, u64), DbErr> {
        use sea_orm::{PaginatorTrait, QueryOrder};
        let paginator = Entity::find()
            .filter(Column::UserId.eq(user_id))
            .order_by_desc(Column::Ts)
            .order_by_desc(Column::Id)
            .paginate(db, per_page);
        let total = paginator.num_items().await?;
        Ok((paginator.fetch_page(page - 1).await?, total))
    }

    pub async fn insert_event(db: &DatabaseConnection, am: ActiveModel) -> Result<(), DbErr> {
        Entity::insert(am).exec(db).await.map(|_| ())
    }

    pub async fn count_event_type_since(
        db: &DatabaseConnection,
        event_type: &EventType,
        since: DateTime<Utc>,
    ) -> Result<u64, DbErr> {
        let et = event_type.to_string();
        Entity::find()
            .filter(Column::EventType.eq(et))
            .filter(Column::Ts.gte(since))
            .count(db)
            .await
    }

    pub async fn record(
        db: &DatabaseConnection,
        event_type: &EventType,
        payload: AuthEventPayload,
    ) -> Result<(), DbErr> {
        let am = ActiveModel {
            id: NotSet,
            ts: NotSet,
            event_type: Set(event_type.to_string()),
            user_id: Set(payload.user_id),
            email: Set(payload.email),
            ip: Set(payload.ip),
            user_agent: Set(payload.user_agent),
            reason: Set(payload.reason),
            meta: Set(payload.meta),
        };

        Self::insert_event(db, am).await
    }
}

#[derive(Clone, Debug, Default)]
pub struct AuthEventPayload {
    pub user_id: Option<i32>,
    pub email: Option<String>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub reason: Option<String>,
    pub meta: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventType {
    PasswordResetRequest,
    PasswordReset,
    LoginFailed,
    LoginSucceeded,
    Logout,
    TokenRefresh,
    TokenRefreshFailed,
    Other(String),
}

impl From<&str> for EventType {
    fn from(s: &str) -> Self {
        match s {
            "password_reset_request" => EventType::PasswordResetRequest,
            "password_reset" => EventType::PasswordReset,
            "login_failed" => EventType::LoginFailed,
            "login_succeeded" => EventType::LoginSucceeded,
            "logout" => EventType::Logout,
            "token_refresh" => EventType::TokenRefresh,
            "token_refresh_failed" => EventType::TokenRefreshFailed,
            other => EventType::Other(other.to_string()),
        }
    }
}

impl From<String> for EventType {
    fn from(s: String) -> Self {
        EventType::from(s.as_str())
    }
}

impl std::fmt::Display for EventType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            EventType::PasswordResetRequest => "password_reset_request",
            EventType::PasswordReset => "password_reset",
            EventType::LoginFailed => "login_failed",
            EventType::LoginSucceeded => "login_succeeded",
            EventType::Logout => "logout",
            EventType::TokenRefresh => "token_refresh",
            EventType::TokenRefreshFailed => "token_refresh_failed",
            EventType::Other(value) => value,
        })
    }
}
