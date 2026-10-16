use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{extension::postgres::PgExpr, Alias, Expr, Func};
use sea_orm::{ConnectionTrait, DatabaseBackend, DbErr, ExprTrait, QueryOrder, QuerySelect, Set};

#[derive(Debug, Clone)]
pub struct NewEvent {
    pub user_id: Option<i32>,
    pub provider: String,
    pub event_type: String,
    pub level: String,
    pub message: String,
    pub connection_id: Option<i32>,
    pub payload: Option<Json>,
}

#[derive(Debug, Clone)]
pub struct ListOptions {
    pub provider: Option<String>,
    pub user_id: Option<i32>,
    pub activity_id: Option<i32>,
    pub import_id: Option<i32>,
    pub limit: u64,
}

impl Entity {
    pub async fn associate_activity(
        db: &impl ConnectionTrait,
        event: &crate::strava::events::StravaEvent,
        activity_id: i32,
    ) -> Result<(), DbErr> {
        let row = Self::find()
            .filter(Column::Provider.eq("strava"))
            .filter(Column::UserId.eq(event.bike_user_id))
            .filter(Column::EventType.eq("strava.event.accepted"))
            .filter(
                Self::payload_field(db.get_database_backend(), "strava_event_id")
                    .eq(event.strava_event_id.to_string()),
            )
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Strava audit event was not found".into(),
            ))?;
        let mut payload = row.payload.clone().unwrap_or_default();
        payload["activity_id"] = activity_id.into();
        payload["bike_activity_id"] = activity_id.into();
        let mut active: ActiveModel = row.into();
        active.payload = Set(Some(payload));
        active.update(db).await?;
        Ok(())
    }

    pub async fn account_strava_history(
        db: &impl ConnectionTrait,
        user_id: i32,
        limit: u64,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::Provider.eq("strava"))
            .filter(Column::UserId.eq(user_id))
            .filter(Column::EventType.eq("strava.event.accepted"))
            .filter(Self::payload_field(db.get_database_backend(), "parent_event_id").is_null())
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .limit(std::cmp::max(limit, 1))
            .all(db)
            .await
    }

    fn payload_field(backend: DatabaseBackend, key: &str) -> Expr {
        let payload = Column::Payload.into_expr();
        match backend {
            DatabaseBackend::Sqlite => Func::cust(Alias::new("json_extract"))
                .args([payload, Expr::val(format!("$.{key}"))])
                .into(),
            DatabaseBackend::MySql => Func::cust(Alias::new("JSON_UNQUOTE"))
                .arg(
                    Func::cust(Alias::new("JSON_EXTRACT"))
                        .args([payload, Expr::val(format!("$.{key}"))]),
                )
                .into(),
            _ => PgExpr::cast_json_field(payload, key),
        }
    }

    pub async fn record(db: &impl ConnectionTrait, event: NewEvent) -> Result<Model, DbErr> {
        ActiveModel {
            user_id: Set(event.user_id),
            provider: Set(event.provider),
            event_type: Set(event.event_type),
            level: Set(event.level),
            message: Set(event.message),
            connection_id: Set(event.connection_id),
            payload: Set(event.payload),
            ..Default::default()
        }
        .insert(db)
        .await
    }

    pub async fn recent(
        db: &impl ConnectionTrait,
        options: ListOptions,
    ) -> Result<Vec<Model>, DbErr> {
        let mut query = Self::find().order_by_desc(Column::CreatedAt);
        if let Some(provider) = options.provider {
            query = query.filter(Column::Provider.eq(provider));
        }
        if let Some(user_id) = options.user_id {
            query = query.filter(Column::UserId.eq(user_id));
        }
        if let Some(activity_id) = options.activity_id {
            query = query.filter(Self::payload_int_filter(
                db.get_database_backend(),
                "activity_id",
                activity_id,
            ));
        }
        if let Some(import_id) = options.import_id {
            query = query.filter(Self::payload_int_filter(
                db.get_database_backend(),
                "import_id",
                import_id,
            ));
        }
        query.limit(std::cmp::max(options.limit, 1)).all(db).await
    }

    fn payload_int_filter(backend: DatabaseBackend, key: &str, value: i32) -> Expr {
        let field = Self::payload_field(backend, key);
        match backend {
            DatabaseBackend::Sqlite => field.clone().eq(value).or(field.eq(value.to_string())),
            _ => field.eq(value.to_string()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "integration_events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: Option<i32>,
    pub provider: String,
    pub event_type: String,
    pub level: String,
    pub message: String,
    pub connection_id: Option<i32>,
    pub payload: Option<Json>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
