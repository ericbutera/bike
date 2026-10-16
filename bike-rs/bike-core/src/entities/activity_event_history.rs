//! Append-only modification history shared by Strava, manual edits, and imports.
//! Append in the mutation's transaction so a committed change always has history.
//! These records diagnose domain changes; Temporal remains the execution owner.
use super::{activities, strava_activity_states};
use crate::{
    bike_tasks::{ImportExecution, ImportTarget},
    strava::native::ActivityChange,
};
use sea_orm::{
    entity::prelude::*, sea_query::OnConflict, DatabaseTransaction, QueryOrder, QuerySelect, Set,
};
#[cfg(test)]
mod tests;

const RECENT_HISTORY_LIMIT: u64 = 100;

/// One archived change to an activity, including its causal context and snapshots.
/// Keep historical Bike IDs after deletion. Recording bytes stay in object storage;
/// snapshots retain summary fields and source metadata, not duplicated recordings.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_event_history")]
pub struct Model {
    /// Source-qualified idempotency key, independent of workflow retention.
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub activity_id: Option<i32>,
    pub user_id: i32,
    pub source: String,
    /// Source event time, never compared with local timestamps for ordering.
    pub occurred_at: DateTimeUtc,
    /// Local archive write time; this is an audit field, not an event sequence.
    pub recorded_at: DateTimeUtc,
    pub before_json: Option<Json>,
    pub after_json: Option<Json>,
    /// Source event/command, actor, request/trace context, and workflow references.
    pub context_json: Json,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

/// A complete domain change prepared before persistence; no scheduling state.
pub struct Event {
    pub id: String,
    pub activity_id: Option<i32>,
    pub user_id: i32,
    pub source: String,
    pub occurred_at: DateTimeUtc,
    pub before: Option<Json>,
    pub after: Option<Json>,
    pub context: Json,
}

impl Event {
    pub fn strava(
        change: &ActivityChange,
        state: &strava_activity_states::Model,
        before: Option<&activities::Model>,
        after: Option<&activities::Model>,
    ) -> Result<Self, DbErr> {
        Ok(Self {
            id: format!("strava:{}", change.event.strava_event_id),
            activity_id: change.bike_activity_id.or(state.bike_activity_id),
            user_id: change.event.bike_user_id,
            source: "strava".into(),
            occurred_at: chrono::DateTime::from_timestamp(change.event.event_time, 0)
                .ok_or_else(|| DbErr::Custom("Invalid Strava event time".into()))?,
            before: before.map(snapshot).transpose()?,
            after: after.map(snapshot).transpose()?,
            context: serde_json::json!({
                "event": change.event, "status": change.status,
                "workflow_id": change.event.workflow_id(), "strava_state": state,
            }),
        })
    }

    pub fn manual(before: &activities::Model, after: &activities::Model) -> Result<Self, DbErr> {
        Ok(Self {
            id: format!("manual:{}", uuid::Uuid::new_v4()),
            activity_id: Some(after.id),
            user_id: after.user_id,
            source: "manual".into(),
            occurred_at: chrono::Utc::now(),
            before: Some(snapshot(before)?),
            after: Some(snapshot(after)?),
            context: serde_json::json!({
                "actor_user_id": after.user_id,
                "trace_context": crate::observability::inject_current_trace_context(),
            }),
        })
    }

    /// Concrete native import adapters append this in their domain-write transaction.
    pub fn import(
        execution: &ImportExecution,
        before: Option<&activities::Model>,
        after: &activities::Model,
    ) -> Result<Self, DbErr> {
        let source = match execution.command.target {
            ImportTarget::NewActivity => "import",
            ImportTarget::ExistingActivity { .. } => "reprocess",
        };
        Ok(Self {
            id: import_event_id(execution),
            activity_id: Some(after.id),
            user_id: execution.command.source.user_id,
            source: source.into(),
            occurred_at: execution
                .command
                .source
                .cause
                .as_ref()
                .map_or(after.updated_at, |cause| cause.received_at),
            before: before.map(snapshot).transpose()?,
            after: Some(snapshot(after)?),
            context: serde_json::to_value(execution).map_err(json_error)?,
        })
    }
}

fn snapshot(activity: &activities::Model) -> Result<Json, DbErr> {
    let mut snapshot = serde_json::to_value(activity).map_err(json_error)?;
    if let Some(fields) = snapshot.as_object_mut() {
        fields.remove("derived_data_json");
        fields.remove("heart_rate_zones_json");
        fields.insert(
            "source_metadata".into(),
            serde_json::to_value(
                activity
                    .derived_data_json
                    .as_ref()
                    .and_then(|data| data.source_metadata()),
            )
            .map_err(json_error)?,
        );
    }
    Ok(snapshot)
}

fn json_error(error: serde_json::Error) -> DbErr {
    DbErr::Custom(error.to_string())
}

impl Model {
    /// The audit event is also the natural command key for a committed import.
    pub async fn import_refresh(
        db: &impl ConnectionTrait,
        execution: &ImportExecution,
    ) -> Result<
        Option<crate::activity_reprocessing::RefreshScope>,
        crate::workflow_error::WorkflowError,
    > {
        let Some(event) = Entity::find_by_id(import_event_id(execution))
            .one(db)
            .await?
        else {
            return Ok(None);
        };
        let original: ImportExecution =
            serde_json::from_value(event.context_json.clone()).map_err(json_error)?;
        if !original.matches_request(execution) {
            return Err(crate::workflow_error::WorkflowError::conflict(
                "Import command already identifies different work",
            ));
        }
        serde_json::from_value(event.context_json["refresh_scope"].clone())
            .map(Some)
            .map_err(|error| json_error(error).into())
    }

    pub async fn record_strava(
        db: &DatabaseTransaction,
        change: &ActivityChange,
        state: &strava_activity_states::Model,
        before: Option<&activities::Model>,
        after: Option<&activities::Model>,
    ) -> Result<(), DbErr> {
        Self::append(db, Event::strava(change, state, before, after)?).await
    }

    /// The caller supplies the same transaction used for the domain mutation.
    pub async fn append(db: &DatabaseTransaction, event: Event) -> Result<(), DbErr> {
        Self::append_once(db, event).await?;
        Ok(())
    }

    /// A command reused concurrently for a different target must roll back that
    /// target's mutation when the audit key is already taken.
    pub async fn append_import(
        db: &DatabaseTransaction,
        event: Event,
    ) -> Result<(), crate::workflow_error::WorkflowError> {
        if Self::append_once(db, event).await? == 0 {
            return Err(crate::workflow_error::WorkflowError::conflict(
                "Import command was committed by another request",
            ));
        }
        Ok(())
    }

    async fn append_once(db: &DatabaseTransaction, event: Event) -> Result<u64, DbErr> {
        Entity::insert(ActiveModel {
            id: Set(event.id),
            activity_id: Set(event.activity_id),
            user_id: Set(event.user_id),
            source: Set(event.source),
            occurred_at: Set(event.occurred_at),
            recorded_at: Set(chrono::Utc::now()),
            before_json: Set(event.before),
            after_json: Set(event.after),
            context_json: Set(event.context),
        })
        .on_conflict(OnConflict::column(Column::Id).do_nothing().to_owned())
        .exec_without_returning(db)
        .await
    }

    pub async fn recent(
        db: &impl ConnectionTrait,
        user_id: i32,
        activity_id: i32,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityId.eq(activity_id))
            .order_by_desc(Column::RecordedAt)
            .order_by_desc(Column::Id)
            .limit(RECENT_HISTORY_LIMIT)
            .all(db)
            .await
    }
}

fn import_event_id(execution: &ImportExecution) -> String {
    let source = match execution.command.target {
        ImportTarget::NewActivity => "import",
        ImportTarget::ExistingActivity { .. } => "reprocess",
    };
    format!(
        "{source}:user-{}:{}",
        execution.command.source.user_id, execution.command.source.command_id
    )
}
