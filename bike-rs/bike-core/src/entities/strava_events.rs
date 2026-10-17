mod contracts;
mod event;
pub use contracts::*;
pub use event::{webhook_operation, SourceVersion, StravaEvent, StravaOperation, StravaWebhookEvent};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ActivityMutation {
    event: StravaEvent,
    bike_activity_id: Option<i32>,
    import_id: Option<i32>,
    segment_ids: Vec<i32>,
    dirty_from: Option<chrono::NaiveDate>,
    removed_source: Option<String>,
}

impl From<&ActivityChange> for ActivityMutation {
    fn from(change: &ActivityChange) -> Self {
        Self {
            event: change.event.clone(),
            bike_activity_id: change.bike_activity_id,
            import_id: change.import_id,
            segment_ids: change.segment_ids.clone(),
            dirty_from: change.dirty_from,
            removed_source: change.removed_source.clone(),
        }
    }
}

impl ActivityMutation {
    fn into_change(self, status: ActivityStatus) -> ActivityChange {
        ActivityChange {
            event: self.event,
            status,
            bike_activity_id: self.bike_activity_id,
            import_id: self.import_id,
            segment_ids: self.segment_ids,
            dirty_from: self.dirty_from,
            removed_source: self.removed_source,
        }
    }
}
use sea_orm::{
    entity::prelude::*, sea_query::OnConflict, FromQueryResult, QueryOrder, QuerySelect, Set, Statement,
    TransactionTrait,
};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "strava_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub event_key: String,
    pub correlation_id: String,
    pub parent_event_id: Option<String>,
    pub user_id: i32,
    pub customer_id: Uuid,
    pub athlete_id: i64,
    pub activity_id: Option<i64>,
    pub workflow_id: String,
    pub event: Json,
    pub status: ActivityStatus,
    pub mutation: Option<Json>,
    pub accepted_at: DateTimeUtc,
    pub submitted_at: Option<DateTimeUtc>,
    pub next_attempt_at: DateTimeUtc,
    pub handoff_error: Option<String>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, PartialEq, FromQueryResult)]
pub struct HandoffBacklog {
    pub pending: i64,
    pub oldest_accepted_at: Option<DateTimeUtc>,
}

impl HandoffBacklog {
    pub async fn load(db: &impl ConnectionTrait) -> Result<Self, DbErr> {
        Self::find_by_statement(Statement::from_string(
            db.get_database_backend(),
            "SELECT COUNT(*)::bigint AS pending, MIN(accepted_at) AS oldest_accepted_at
             FROM strava_events WHERE submitted_at IS NULL",
        ))
        .one(db)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("Strava handoff backlog was not returned".into()))
    }
}

impl Model {
    pub async fn collection_counts(db: &impl ConnectionTrait, event: &StravaEvent) -> Result<(i32, i32), DbErr> {
        use sea_orm::{DatabaseBackend, FromQueryResult, Statement};
        #[derive(FromQueryResult)]
        struct Counts {
            applied: i32,
            other: i32,
        }
        let counts = Counts::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT COUNT(*) FILTER (WHERE status=$3)::integer AS applied,
                COUNT(*) FILTER (WHERE status<>$3)::integer AS other
             FROM strava_events WHERE parent_event_id=$1 AND user_id=$2",
            [
                event.strava_event_id.to_string().into(),
                event.bike_user_id.into(),
                ActivityStatus::Applied.into(),
            ],
        ))
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound(
            "Strava collection counts were not returned".into(),
        ))?;
        Ok((counts.applied, counts.other))
    }
    pub async fn accept(db: &DatabaseConnection, event: &StravaEvent) -> Result<Self, DbErr> {
        let txn = db.begin().await?;
        let id = event.strava_event_id.to_string();
        let key = if event.webhook.is_some() {
            event.webhook_key().map_err(|error| DbErr::Custom(error.to_string()))?
        } else {
            id.clone()
        };
        let inserted = Entity::insert(ActiveModel {
            id: Set(id),
            event_key: Set(key.clone()),
            correlation_id: Set(event.correlation_id.to_string()),
            parent_event_id: Set(event.parent_event_id.map(|id| id.to_string())),
            user_id: Set(event.bike_user_id),
            customer_id: Set(event.bike_customer_id),
            athlete_id: Set(event.strava_athlete_id),
            activity_id: Set(event.strava_activity_id),
            workflow_id: Set(event.workflow_id()),
            event: Set(serde_json::to_value(event).map_err(|error| DbErr::Custom(error.to_string()))?),
            mutation: Set(None),
            status: Set(ActivityStatus::Pending),
            accepted_at: Set(chrono::Utc::now()),
            submitted_at: Set(None),
            next_attempt_at: Set(chrono::Utc::now()),
            handoff_error: Set(None),
        })
        .on_conflict(OnConflict::column(Column::EventKey).do_nothing().to_owned())
        .exec_without_returning(&txn)
        .await?;
        let row = Entity::find()
            .filter(Column::EventKey.eq(key))
            .one(&txn)
            .await?
            .ok_or(DbErr::RecordNotFound("Strava event was not stored".into()))?;
        if inserted > 0 {
            super::integration_events::Entity::record(
                &txn,
                super::integration_events::NewEvent {
                    user_id: Some(event.bike_user_id),
                    provider: "strava".into(),
                    event_type: "strava.event.accepted".into(),
                    level: "info".into(),
                    message: format!("Strava {:?} requested.", event.operation),
                    connection_id: Some(event.connection_id),
                    payload: Some(serde_json::json!({
                        "operation":event.operation, "strava_event_id":row.id,
                        "correlation_id":row.correlation_id, "parent_event_id":row.parent_event_id,
                        "bike_customer_id":row.customer_id,"bike_user_id":row.user_id,
                        "strava_athlete_id":row.athlete_id,"strava_activity_id":row.activity_id,
                        "temporal_namespace":std::env::var("TEMPORAL_NAMESPACE").unwrap_or_else(|_| "bike".into()),
                        "temporal_workflow_id":row.workflow_id,
                    })),
                },
            )
            .await?;
        }
        txn.commit().await?;
        Ok(row)
    }

    pub async fn pending(db: &impl ConnectionTrait) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::SubmittedAt.is_null())
            .filter(Column::NextAttemptAt.lte(chrono::Utc::now()))
            .order_by_asc(Column::NextAttemptAt)
            .limit(100)
            .all(db)
            .await
    }

    pub async fn submitted(self, db: &impl ConnectionTrait) -> Result<(), DbErr> {
        let mut active: ActiveModel = self.into();
        active.submitted_at = Set(Some(chrono::Utc::now()));
        active.handoff_error = Set(None);
        active.update(db).await?;
        Ok(())
    }

    pub async fn retry_handoff(self, db: &impl ConnectionTrait, error: String) -> Result<(), DbErr> {
        let mut active: ActiveModel = self.into();
        active.next_attempt_at = Set(chrono::Utc::now() + chrono::Duration::seconds(30));
        active.handoff_error = Set(Some(error));
        active.update(db).await?;
        Ok(())
    }

    pub async fn stored_mutation(
        db: &impl ConnectionTrait,
        event: &StravaEvent,
    ) -> Result<Option<ActivityChange>, DbErr> {
        let row = Entity::find_by_id(event.strava_event_id.to_string())
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Strava event was not accepted".into()))?;
        if !row.belongs_to(event) {
            return Err(DbErr::Custom(
                "Strava event identity does not match stored acceptance".into(),
            ));
        }
        let stored: StravaEvent =
            serde_json::from_value(row.event.clone()).map_err(|error| DbErr::Custom(error.to_string()))?;
        if !stored.has_same_request(event) {
            return Err(DbErr::Custom(
                "Strava event contract does not match stored acceptance".into(),
            ));
        }
        row.mutation
            .map(|value| {
                serde_json::from_value::<ActivityMutation>(value).map(|mutation| mutation.into_change(row.status))
            })
            .transpose()
            .map_err(|error| DbErr::Custom(error.to_string()))
    }

    fn belongs_to(&self, event: &StravaEvent) -> bool {
        self.has_same_customer(event) && self.has_same_activity(event)
    }

    fn has_same_customer(&self, event: &StravaEvent) -> bool {
        self.user_id == event.bike_user_id && self.customer_id == event.bike_customer_id
    }

    fn has_same_activity(&self, event: &StravaEvent) -> bool {
        self.athlete_id == event.strava_athlete_id && self.activity_id == event.strava_activity_id
    }

    pub async fn record_mutation(db: &impl ConnectionTrait, change: &ActivityChange) -> Result<(), DbErr> {
        if let Some(id) = change.bike_activity_id {
            super::integration_events::Entity::associate_activity(db, &change.event, id).await?;
        }
        let value =
            serde_json::to_value(ActivityMutation::from(change)).map_err(|error| DbErr::Custom(error.to_string()))?;
        Entity::update_many()
            .set(ActiveModel {
                mutation: Set(Some(value)),
                status: Set(change.status),
                ..Default::default()
            })
            .filter(Column::Id.eq(change.event.strava_event_id.to_string()))
            .exec(db)
            .await?;
        Ok(())
    }
}
