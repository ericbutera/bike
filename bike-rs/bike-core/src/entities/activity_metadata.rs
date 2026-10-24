//! Typed editable activity metadata and ordered recorded laps.
use crate::{
    activity_data::{ActivityLap, ActivitySourceMetadata},
    bike_tasks::{ActivityTask, Priority},
    workflow_error::WorkflowError,
};
use sea_orm::entity::prelude::*;
use sea_orm::{
    sea_query::OnConflict, DatabaseBackend, FromQueryResult, QueryOrder, QuerySelect, QueryTrait, Set, Statement,
    TransactionTrait,
};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_metadata")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i32,
    pub description: Option<String>,
    pub location: Option<String>,
    pub private_notes: Option<String>,
    pub gear_id: Option<String>,
    pub gear_name: Option<String>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug)]
pub struct SavedDetails {
    pub metadata: Model,
    pub laps: Vec<ActivityLap>,
}

#[derive(FromQueryResult)]
struct PendingActivity {
    activity_id: i32,
    user_id: i32,
}

impl Model {
    pub async fn load(
        db: &impl ConnectionTrait,
        activity_id: i32,
        user_id: i32,
    ) -> Result<Option<SavedDetails>, DbErr> {
        let metadata = Entity::find_by_id(activity_id)
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
            .await?;
        let Some(metadata) = metadata else {
            return Ok(None);
        };
        let laps = laps::Model::for_activity(db, activity_id).await?;
        Ok(Some(SavedDetails { metadata, laps }))
    }

    /// The caller holds the activity lock or inserted it in this transaction.
    pub async fn store(db: &impl ConnectionTrait, activity: &super::activities::Model) -> Result<(), DbErr> {
        Self::store_metadata(db, activity).await?;
        let data = crate::activity_data::deserialize_derived_activity_data(activity.derived_data_json.as_ref());
        laps::Model::replace(db, activity.id, data.laps).await
    }

    pub async fn sync_edit(
        db: &impl ConnectionTrait,
        before: &super::activities::Model,
        after: &super::activities::Model,
    ) -> Result<(), DbErr> {
        let before = before
            .derived_data_json
            .as_ref()
            .and_then(|data| data.source_metadata());
        let after_metadata = after.derived_data_json.as_ref().and_then(|data| data.source_metadata());
        if before != after_metadata {
            Self::store(db, after).await?;
        }
        Ok(())
    }

    async fn store_metadata(db: &impl ConnectionTrait, activity: &super::activities::Model) -> Result<(), DbErr> {
        let metadata = activity
            .derived_data_json
            .as_ref()
            .and_then(|data| data.source_metadata())
            .cloned()
            .unwrap_or_default();
        Entity::insert(ActiveModel::from_metadata(activity.id, metadata))
            .on_conflict(
                OnConflict::column(Column::ActivityId)
                    .update_columns([
                        Column::Description,
                        Column::Location,
                        Column::PrivateNotes,
                        Column::GearId,
                        Column::GearName,
                    ])
                    .to_owned(),
            )
            .exec_without_returning(db)
            .await?;
        Ok(())
    }

    pub async fn pending_tasks(db: &impl ConnectionTrait, after_id: i32) -> Result<Vec<ActivityTask>, DbErr> {
        let rows = PendingActivity::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT a.id AS activity_id, a.user_id FROM activities a
             WHERE a.id > $1 AND NOT EXISTS (SELECT 1 FROM activity_metadata m WHERE m.activity_id = a.id)
             ORDER BY a.id LIMIT 16",
            [after_id.into()],
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

    pub async fn backfill(db: &DatabaseConnection, task: ActivityTask) -> Result<(), WorkflowError> {
        let txn = db.begin().await?;
        if let Some(activity) = super::activities::Model::lock_owned(&txn, task.activity_id, task.user_id).await? {
            Self::store(&txn, &activity).await?;
        }
        txn.commit().await?;
        Ok(())
    }
}

impl ActiveModel {
    fn from_metadata(activity_id: i32, metadata: ActivitySourceMetadata) -> Self {
        let (gear_id, gear_name) = metadata.gear.map_or((None, None), |gear| (Some(gear.id), gear.name));
        Self {
            activity_id: Set(activity_id),
            description: Set(metadata.description),
            location: Set(metadata.location),
            private_notes: Set(metadata.private_notes),
            gear_id: Set(gear_id),
            gear_name: Set(gear_name),
        }
    }
}

pub mod laps {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "activity_laps")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub activity_id: i32,
        #[sea_orm(primary_key, auto_increment = false)]
        pub position: i32,
        pub lap_index: i32,
        pub title: String,
        pub start_offset_seconds: Option<i32>,
        pub duration_seconds: Option<i32>,
        pub distance_meters: Option<f64>,
        pub elevation_gain_meters: Option<f64>,
        pub elevation_loss_meters: Option<f64>,
        pub average_speed_mps: Option<f64>,
        pub max_speed_mps: Option<f64>,
        pub average_heart_rate_bpm: Option<i32>,
        pub max_heart_rate_bpm: Option<i32>,
        pub average_cadence_rpm: Option<i32>,
        pub max_cadence_rpm: Option<i32>,
        pub calories: Option<i32>,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}

    impl Model {
        pub(super) async fn for_activity(
            db: &impl ConnectionTrait,
            activity_id: i32,
        ) -> Result<Vec<ActivityLap>, DbErr> {
            Ok(Entity::find()
                .filter(Column::ActivityId.eq(activity_id))
                .order_by_asc(Column::Position)
                .all(db)
                .await?
                .into_iter()
                .map(ActivityLap::from)
                .collect())
        }

        pub(super) async fn replace(
            db: &impl ConnectionTrait,
            activity_id: i32,
            laps: Vec<ActivityLap>,
        ) -> Result<(), DbErr> {
            let rows: Vec<_> = laps
                .into_iter()
                .enumerate()
                .map(|(position, lap)| ActiveModel::from_lap(activity_id, position, lap))
                .collect::<Result<_, _>>()?;
            Entity::delete_many()
                .filter(Column::ActivityId.eq(activity_id))
                .exec(db)
                .await?;
            for batch in rows.chunks(500) {
                Entity::insert_many(batch.iter().cloned())
                    .exec_without_returning(db)
                    .await?;
            }
            Ok(())
        }
    }

    impl ActiveModel {
        fn from_lap(activity_id: i32, position: usize, lap: ActivityLap) -> Result<Self, DbErr> {
            Ok(Self {
                activity_id: Set(activity_id),
                position: Set(i32::try_from(position).map_err(|_| DbErr::Custom("Too many activity laps".into()))?),
                lap_index: Set(lap.lap_index),
                title: Set(lap.title),
                start_offset_seconds: Set(lap.start_offset_seconds),
                duration_seconds: Set(lap.duration_seconds),
                distance_meters: Set(lap.distance_meters),
                elevation_gain_meters: Set(lap.elevation_gain_meters),
                elevation_loss_meters: Set(lap.elevation_loss_meters),
                average_speed_mps: Set(lap.average_speed_mps),
                max_speed_mps: Set(lap.max_speed_mps),
                average_heart_rate_bpm: Set(lap.average_heart_rate_bpm),
                max_heart_rate_bpm: Set(lap.max_heart_rate_bpm),
                average_cadence_rpm: Set(lap.average_cadence_rpm),
                max_cadence_rpm: Set(lap.max_cadence_rpm),
                calories: Set(lap.calories),
            })
        }
    }

    impl From<Model> for ActivityLap {
        fn from(lap: Model) -> Self {
            Self {
                lap_index: lap.lap_index,
                title: lap.title,
                start_offset_seconds: lap.start_offset_seconds,
                duration_seconds: lap.duration_seconds,
                distance_meters: lap.distance_meters,
                elevation_gain_meters: lap.elevation_gain_meters,
                elevation_loss_meters: lap.elevation_loss_meters,
                average_speed_mps: lap.average_speed_mps,
                max_speed_mps: lap.max_speed_mps,
                average_heart_rate_bpm: lap.average_heart_rate_bpm,
                max_heart_rate_bpm: lap.max_heart_rate_bpm,
                average_cadence_rpm: lap.average_cadence_rpm,
                max_cadence_rpm: lap.max_cadence_rpm,
                calories: lap.calories,
            }
        }
    }
}

#[cfg(test)]
mod tests;
