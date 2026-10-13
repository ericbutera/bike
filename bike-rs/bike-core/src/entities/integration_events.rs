use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{extension::postgres::PgExpr, Alias, Expr, Func};
use sea_orm::{
    Condition, ConnectionTrait, DatabaseBackend, DbErr, ExprTrait, QueryOrder, QuerySelect, Set,
};

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
    pub async fn account_strava_history(
        db: &impl ConnectionTrait,
        user_id: i32,
        limit: u64,
    ) -> Result<Vec<Model>, DbErr> {
        let backend = db.get_database_backend();
        let collection = Condition::all()
            .add(Self::payload_field(backend, "action").is_in(["sync", "backfill"]))
            .add(
                Condition::any()
                    .add(Column::EventType.eq("workflow.accepted"))
                    .add(
                        Condition::all()
                            .add(Column::EventType.eq("workflow.stage"))
                            .add(
                                Self::payload_field(backend, "stage")
                                    .is_in(["discover", "children", "complete"]),
                            ),
                    ),
            );
        let relevant = Condition::any()
            .add(Column::EventType.starts_with("sync."))
            .add(Column::EventType.starts_with("sync_"))
            .add(collection)
            .add(Self::activity_change_filter(backend, "apply", "applied"))
            .add(Self::activity_change_filter(backend, "delete", "deleted"))
            .add(
                Condition::all()
                    .add(Column::EventType.eq("workflow.stage.failed"))
                    .add(
                        Self::payload_field(backend, "stage")
                            .is_in(["discover", "children", "complete"]),
                    ),
            )
            .add(
                Condition::all()
                    .add(Column::EventType.eq("webhook.activity_delete"))
                    .add(Column::Level.eq("success")),
            );
        Self::find()
            .filter(Column::Provider.eq("strava"))
            .filter(Column::UserId.eq(user_id))
            .filter(relevant)
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .limit(std::cmp::max(limit, 1))
            .all(db)
            .await
    }

    fn activity_change_filter(backend: DatabaseBackend, stage: &str, outcome: &str) -> Condition {
        Condition::all()
            .add(Column::EventType.eq("workflow.stage"))
            .add(Self::payload_field(backend, "stage").eq(stage))
            .add(Self::payload_field(backend, "outcome").eq(outcome))
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

    pub async fn prune_strava_workflows(
        db: &impl ConnectionTrait,
        ids: &[String],
    ) -> Result<u64, DbErr> {
        use sea_orm::{ColumnTrait, QueryFilter};
        let payload = Column::Payload.into_expr();
        let workflow = match db.get_database_backend() {
            DatabaseBackend::Sqlite => Func::cust(Alias::new("json_extract"))
                .args([payload, Expr::val("$.workflow_id")])
                .into(),
            DatabaseBackend::MySql => Func::cust(Alias::new("JSON_UNQUOTE"))
                .arg(
                    Func::cust(Alias::new("JSON_EXTRACT"))
                        .args([payload, Expr::val("$.workflow_id")]),
                )
                .into(),
            _ => PgExpr::cast_json_field(payload, "workflow_id"),
        };
        let mut deleted = 0;
        for batch in ids.chunks(500) {
            deleted += Self::delete_many()
                .filter(Column::Provider.eq("strava"))
                .filter(
                    Condition::any()
                        .add(Column::EventType.starts_with("workflow."))
                        .add(Column::EventType.eq("sync.completed")),
                )
                .filter(workflow.clone().is_in(batch.iter().cloned()))
                .exec(db)
                .await?
                .rows_affected;
        }
        Ok(deleted)
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

    /// The workflow's connection lease serializes completion and retry records.
    pub async fn record_workflow_once(
        db: &impl ConnectionTrait,
        workflow_id: &str,
        event: NewEvent,
    ) -> Result<Model, DbErr> {
        if let Some(existing) = Self::find()
            .filter(Column::Provider.eq(&event.provider))
            .filter(Column::UserId.eq(event.user_id))
            .filter(Column::EventType.eq(&event.event_type))
            .filter(Self::payload_field(db.get_database_backend(), "workflow_id").eq(workflow_id))
            .one(db)
            .await?
        {
            return Ok(existing);
        }
        Self::record(db, event).await
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

    fn history_event(event_type: &str, payload: Json) -> NewEvent {
        NewEvent {
            user_id: Some(1),
            provider: "strava".into(),
            event_type: event_type.into(),
            level: "info".into(),
            message: "Internal diagnostic message".into(),
            connection_id: Some(1),
            payload: Some(payload),
        }
    }

    #[tokio::test]
    async fn account_history_filters_processing_noise_before_limiting_and_scopes_events() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute(&Schema::new(db.get_database_backend()).create_table_from_entity(Entity))
            .await
            .unwrap();
        let mut foreign = history_event("sync.completed", serde_json::json!({"imported_count": 5}));
        foreign.user_id = Some(2);
        let mut other_provider = history_event("sync.completed", serde_json::json!({}));
        other_provider.provider = "other".into();
        for event in [
            history_event("sync.succeeded", serde_json::json!({"imported_count": 2})),
            history_event(
                "workflow.accepted",
                serde_json::json!({"action":"sync","workflow_id":"old"}),
            ),
            history_event(
                "workflow.stage",
                serde_json::json!({"action":"sync","stage":"complete","outcome":"completed","workflow_id":"old"}),
            ),
            history_event(
                "workflow.stage",
                serde_json::json!({"action":"upsert","stage":"apply","outcome":"applied","activity_id":42}),
            ),
            history_event(
                "workflow.accepted",
                serde_json::json!({"action":"sync","workflow_id":"new"}),
            ),
            foreign,
            other_provider,
        ] {
            Entity::record(&db, event).await.unwrap();
        }
        for index in 0..30 {
            Entity::record(
                &db,
                history_event(
                    "workflow.stage",
                    serde_json::json!({
                        "action":"upsert","stage":"apply","outcome":"unchanged","activity_id":index,
                    }),
                ),
            )
            .await
            .unwrap();
        }
        let events = crate::integration_events_service::list_account_strava_events(&db, 1, 3)
            .await
            .unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].payload.as_ref().unwrap()["workflow_id"], "new");
        assert_eq!(events[1].payload.as_ref().unwrap()["activity_id"], 42);
        assert_eq!(events[2].payload.as_ref().unwrap()["stage"], "complete");
        assert!(events
            .iter()
            .all(|event| event.user_id == Some(1) && event.provider == "strava"));
        let retained = Entity::account_strava_history(&db, 1, 10).await.unwrap();
        assert_eq!(retained.len(), 5);
        assert!(retained
            .iter()
            .any(|event| event.event_type == "sync.succeeded"));
        assert_eq!(
            Entity::account_strava_history(&db, 1, 0)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn account_history_keeps_backfill_failures_and_applied_removals() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute(&Schema::new(db.get_database_backend()).create_table_from_entity(Entity))
            .await
            .unwrap();
        let mut removed = history_event(
            "webhook.activity_delete",
            serde_json::json!({"activity_id":42}),
        );
        removed.level = "success".into();
        for event in [
            history_event(
                "workflow.accepted",
                serde_json::json!({"action":"backfill"}),
            ),
            history_event(
                "workflow.stage",
                serde_json::json!({"action":"backfill","stage":"discover"}),
            ),
            history_event(
                "workflow.stage.failed",
                serde_json::json!({"stage":"children"}),
            ),
            history_event(
                "workflow.stage",
                serde_json::json!({"stage":"delete","outcome":"deleted","activity_id":42}),
            ),
            removed,
            history_event("workflow.accepted", serde_json::json!({"action":"upsert"})),
            history_event(
                "workflow.stage",
                serde_json::json!({"stage":"delete","outcome":"already_absent"}),
            ),
            history_event(
                "workflow.stage",
                serde_json::json!({"stage":"apply","outcome":"excluded"}),
            ),
            history_event(
                "workflow.stage.failed",
                serde_json::json!({"stage":"activity"}),
            ),
        ] {
            Entity::record(&db, event).await.unwrap();
        }
        let events = Entity::account_strava_history(&db, 1, 25).await.unwrap();
        assert_eq!(events.len(), 5);
        assert!(events
            .iter()
            .any(|event| event.event_type == "workflow.stage.failed"));
        assert!(events
            .iter()
            .any(|event| event.event_type == "webhook.activity_delete"));
    }

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
