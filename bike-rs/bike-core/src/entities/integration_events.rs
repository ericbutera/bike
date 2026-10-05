use async_trait::async_trait;
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
        let payload = Column::Payload.into_expr();
        match backend {
            DatabaseBackend::Sqlite => {
                let field = Func::cust(Alias::new("json_extract"))
                    .args([payload, Expr::val(format!("$.{key}"))]);
                field.clone().eq(value).or(field.eq(value.to_string()))
            }
            DatabaseBackend::MySql => Func::cust(Alias::new("JSON_UNQUOTE"))
                .arg(
                    Func::cust(Alias::new("JSON_EXTRACT"))
                        .args([payload, Expr::val(format!("$.{key}"))]),
                )
                .eq(value.to_string()),
            _ => PgExpr::cast_json_field(payload, key).eq(value.to_string()),
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
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert {
            self.created_at = Set(Utc::now());
        }

        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, Schema};

    #[tokio::test]
    async fn sqlite_filters_numeric_and_string_payload_ids_and_scopes_events() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute(&Schema::new(db.get_database_backend()).create_table_from_entity(Entity))
            .await
            .unwrap();
        for (user_id, provider, payload) in [
            (
                1,
                "strava",
                serde_json::json!({"activity_id":42,"import_id":7}),
            ),
            (
                1,
                "strava",
                serde_json::json!({"activity_id":"42","import_id":"7"}),
            ),
            (
                1,
                "strava",
                serde_json::json!({"strava_activity_id":42,"import_id":7}),
            ),
            (
                2,
                "strava",
                serde_json::json!({"activity_id":42,"import_id":7}),
            ),
            (
                1,
                "provider'quoted",
                serde_json::json!({"activity_id":42,"import_id":7}),
            ),
        ] {
            ActiveModel {
                user_id: Set(Some(user_id)),
                provider: Set(provider.into()),
                event_type: Set("fixture".into()),
                level: Set("info".into()),
                message: Set("Fixture event".into()),
                payload: Set(Some(payload)),
                ..Default::default()
            }
            .insert(&db)
            .await
            .unwrap();
        }
        let options = ListOptions {
            user_id: Some(1),
            provider: Some("strava".into()),
            activity_id: Some(42),
            import_id: Some(7),
            limit: 100,
        };
        let events = Entity::recent(&db, options.clone()).await.unwrap();
        assert_eq!(events.len(), 2);
        assert!(events[0].created_at >= events[1].created_at);
        assert_eq!(
            Entity::recent(
                &db,
                ListOptions {
                    limit: 0,
                    ..options.clone()
                }
            )
            .await
            .unwrap()
            .len(),
            1
        );
        assert_eq!(
            Entity::recent(
                &db,
                ListOptions {
                    provider: Some("provider'quoted".into()),
                    ..options
                }
            )
            .await
            .unwrap()
            .len(),
            1
        );
    }
}
