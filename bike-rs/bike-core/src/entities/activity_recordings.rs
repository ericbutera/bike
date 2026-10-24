//! Current normalized recording artifacts, scoped through their owning activity.
use crate::{
    activity_data::{ActivityChartPoint, ActivityDerivedData, ActivityRoutePoint},
    activity_recording::RecordingContext,
    bike_tasks::{ActivityTask, Priority},
    object_store::{ObjectOwner, ObjectRef, ObjectStore},
    workflow_error::WorkflowError,
};
use sea_orm::entity::prelude::*;
use sea_orm::{
    sea_query::OnConflict, DatabaseBackend, FromQueryResult, QuerySelect, QueryTrait, Set, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};

pub const FORMAT_VERSION: i32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recording {
    pub format_version: i32,
    pub context: RecordingContext,
    pub chart_points: Vec<ActivityChartPoint>,
    pub route_points: Vec<ActivityRoutePoint>,
}

impl From<ActivityDerivedData> for Recording {
    fn from(data: ActivityDerivedData) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            context: data.recording,
            chart_points: data.chart_points,
            route_points: data.route_points,
        }
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_recordings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i32,
    pub format_version: i32,
    pub object_path: String,
    pub object_sha256: String,
    pub object_bytes: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

#[derive(FromQueryResult)]
struct PendingActivity {
    activity_id: i32,
    user_id: i32,
}

impl Model {
    pub async fn for_owned_activity(
        db: &impl ConnectionTrait,
        activity_id: i32,
        user_id: i32,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(activity_id)
            .filter(
                Column::ActivityId.in_subquery(
                    super::activities::Entity::find()
                        .select_only()
                        .column(super::activities::Column::Id)
                        .filter(super::activities::Column::UserId.eq(user_id))
                        .into_query(),
                ),
            )
            .one(db)
            .await
    }

    pub fn object_reference(&self) -> Result<ObjectRef, WorkflowError> {
        Ok(ObjectRef {
            path: self.object_path.clone(),
            sha256: self.object_sha256.clone(),
            bytes: usize::try_from(self.object_bytes)
                .map_err(|_| WorkflowError::invalid_data("Invalid saved recording byte count"))?,
        })
    }

    pub async fn load(
        db: &impl ConnectionTrait,
        objects: &ObjectStore,
        activity_id: i32,
        user_id: i32,
    ) -> Result<Option<Recording>, WorkflowError> {
        let Some(saved) = Self::for_owned_activity(db, activity_id, user_id).await? else {
            return Ok(None);
        };
        if !owned_reference(&saved.object_path, user_id) {
            return Err(WorkflowError::invalid_data(
                "Recording object does not belong to its rider",
            ));
        }
        let recording: Recording = objects
            .for_owner(if saved.object_path.starts_with("strava/") {
                ObjectOwner::Strava
            } else {
                ObjectOwner::BikeTasks
            })
            .read_json(&saved.object_reference()?)
            .await?;
        if recording.format_version != FORMAT_VERSION || saved.format_version != FORMAT_VERSION {
            return Err(WorkflowError::invalid_data("Unsupported normalized recording format"));
        }
        Ok(Some(recording))
    }

    /// Caller holds the activity lock or has just inserted it in the same transaction.
    pub async fn store(
        db: &impl ConnectionTrait,
        objects: &ObjectStore,
        activity: &super::activities::Model,
    ) -> Result<(), WorkflowError> {
        let data = activity
            .derived_data_json
            .clone()
            .ok_or_else(|| WorkflowError::invalid_data("Activity has no saved recording data"))?;
        let recording = Recording::from(ActivityDerivedData::from(data));
        let reference = objects
            .for_owner(ObjectOwner::BikeTasks)
            .transform(&prefix(activity.user_id), &recording)
            .await?;
        Self::store_reference(db, activity, &reference).await
    }

    /// Publish an artifact produced from this activity's normalized parser output.
    pub async fn store_reference(
        db: &impl ConnectionTrait,
        activity: &super::activities::Model,
        reference: &ObjectRef,
    ) -> Result<(), WorkflowError> {
        if !owned_reference(&reference.path, activity.user_id) {
            return Err(WorkflowError::invalid_data(
                "Recording object does not belong to its rider",
            ));
        }
        let current = Self::for_owned_activity(db, activity.id, activity.user_id).await?;
        if current.as_ref().is_some_and(|current| current.matches(reference)) {
            return Ok(());
        }
        Entity::insert(ActiveModel {
            activity_id: Set(activity.id),
            format_version: Set(FORMAT_VERSION),
            object_path: Set(reference.path.clone()),
            object_sha256: Set(reference.sha256.clone()),
            object_bytes: Set(i64::try_from(reference.bytes)
                .map_err(|_| WorkflowError::invalid_data("Recording artifact is too large"))?),
        })
        .on_conflict(
            OnConflict::column(Column::ActivityId)
                .update_columns([
                    Column::FormatVersion,
                    Column::ObjectPath,
                    Column::ObjectSha256,
                    Column::ObjectBytes,
                ])
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
        Ok(())
    }

    fn matches(&self, object: &ObjectRef) -> bool {
        self.format_version == FORMAT_VERSION && self.object_reference().is_ok_and(|current| current == *object)
    }

    pub async fn pending_tasks(db: &impl ConnectionTrait, after_id: i32) -> Result<Vec<ActivityTask>, DbErr> {
        let rows = PendingActivity::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT a.id AS activity_id, a.user_id FROM activities a
             WHERE a.id > $1 AND NOT EXISTS (
                SELECT 1 FROM activity_recordings r WHERE r.activity_id = a.id AND r.format_version = $2
             ) ORDER BY a.id LIMIT 16",
            [after_id.into(), FORMAT_VERSION.into()],
        ))
        .all(db)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| ActivityTask {
                activity_id: row.activity_id,
                user_id: row.user_id,
                priority: Priority::Background,
            })
            .collect())
    }

    /// Convert only the current saved recording, without replaying import effects.
    pub async fn backfill(
        db: &DatabaseConnection,
        objects: &ObjectStore,
        task: ActivityTask,
    ) -> Result<(), WorkflowError> {
        let txn = db.begin().await?;
        if let Some(activity) = super::activities::Model::lock_owned(&txn, task.activity_id, task.user_id).await? {
            Self::store(&txn, objects, &activity).await?;
        }
        txn.commit().await?;
        Ok(())
    }
}

fn prefix(user_id: i32) -> String {
    format!("bike/tasks/recordings/{user_id}")
}

fn owned_reference(path: &str, user_id: i32) -> bool {
    path.starts_with(&format!("{}/", prefix(user_id))) || path.starts_with(&format!("strava/recordings/{user_id}/"))
}

#[cfg(test)]
mod tests;

impl Model {
    /// Matching reads only the normalized output saved by the recording producer.
    pub async fn matching_route(
        db: &impl ConnectionTrait,
        objects: &ObjectStore,
        activity: &super::activities::Model,
    ) -> Result<Vec<ActivityRoutePoint>, WorkflowError> {
        if !activity.is_bike_activity() {
            return Ok(Vec::new());
        }
        let recording = Self::load(db, objects, activity.id, activity.user_id)
            .await?
            .ok_or_else(|| {
                WorkflowError::invalid_data("Activity has no normalized recording; run the recording backfill")
            })?;
        Ok(recording.route_points)
    }
}
