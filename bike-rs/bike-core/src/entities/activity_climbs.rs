use crate::{
    activity_climbs::{Climb, ClimbOutput, DETECTOR_VERSION},
    activity_data::{deserialize_route_point_series, serialize_route_point_series, StoredRoutePointSeries},
    bike_tasks::{ActivityTask, Priority},
    object_store::ObjectRef,
    workflow_error::WorkflowError,
};
use sea_orm::entity::prelude::*;
use sea_orm::{
    sea_query::OnConflict, DatabaseBackend, FromQueryResult, QueryFilter, QueryOrder, Set, Statement, TransactionTrait,
};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_climbs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub climb_number: i32,
    pub start_route_point_index: Option<i32>,
    pub end_route_point_index: Option<i32>,
    pub start_elapsed_seconds: i32,
    pub end_elapsed_seconds: i32,
    pub duration_seconds: i32,
    pub distance_meters: f64,
    pub elevation_gain_meters: f64,
    pub average_grade_percent: Option<f64>,
    pub elevation_loss_meters: Option<f64>,
    pub max_grade_percent: Option<f64>,
    pub category: Option<String>,
    pub route_points_json: StoredRoutePointSeries,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

impl ActiveModel {
    fn from_climb(activity_id: i32, climb: &Climb) -> Self {
        Self {
            activity_id: Set(activity_id),
            climb_number: Set(climb.climb_number),
            start_route_point_index: Set(climb.start_route_point_index),
            end_route_point_index: Set(climb.end_route_point_index),
            start_elapsed_seconds: Set(climb.start_elapsed_seconds),
            end_elapsed_seconds: Set(climb.end_elapsed_seconds),
            duration_seconds: Set(climb.duration_seconds),
            distance_meters: Set(climb.distance_meters),
            elevation_gain_meters: Set(climb.elevation_gain_meters),
            average_grade_percent: Set(climb.average_grade_percent),
            elevation_loss_meters: Set(climb.elevation_loss_meters),
            max_grade_percent: Set(climb.max_grade_percent),
            category: Set(climb.category.clone()),
            route_points_json: Set(serialize_route_point_series(&climb.route_points)),
        }
    }
}

impl Model {
    pub async fn for_activity(db: &impl ConnectionTrait, activity_id: i32) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::ActivityId.eq(activity_id))
            .order_by_asc(Column::ClimbNumber)
            .all(db)
            .await
    }

    pub fn into_climb(self) -> Climb {
        Climb {
            climb_number: self.climb_number,
            start_route_point_index: self.start_route_point_index,
            end_route_point_index: self.end_route_point_index,
            start_elapsed_seconds: self.start_elapsed_seconds,
            end_elapsed_seconds: self.end_elapsed_seconds,
            duration_seconds: self.duration_seconds,
            distance_meters: self.distance_meters,
            elevation_gain_meters: self.elevation_gain_meters,
            average_grade_percent: self.average_grade_percent,
            elevation_loss_meters: self.elevation_loss_meters,
            max_grade_percent: self.max_grade_percent,
            category: self.category,
            route_points: deserialize_route_point_series(Some(&self.route_points_json)),
        }
    }

    pub async fn replace_output(
        db: &DatabaseConnection,
        source: &super::activities::Model,
        climbs: &ClimbOutput,
        reference: &ObjectRef,
    ) -> Result<(), WorkflowError> {
        let txn = db.begin().await?;
        let Some(current) = super::activities::Model::lock_owned(&txn, source.id, source.user_id).await? else {
            return Ok(());
        };
        if current.derived_data_json != source.derived_data_json {
            return Err(WorkflowError::internal(
                "Recording changed while preparing climbs; retry",
            ));
        }
        Entity::delete_many()
            .filter(Column::ActivityId.eq(source.id))
            .exec(&txn)
            .await?;
        output::Model::save_output(&txn, source.id, climbs, reference).await?;
        if !climbs.climbs.is_empty() {
            Entity::insert_many(
                climbs
                    .climbs
                    .iter()
                    .map(|climb| ActiveModel::from_climb(source.id, climb)),
            )
            .exec_without_returning(&txn)
            .await?;
        }
        txn.commit().await?;
        Ok(())
    }
}

pub mod output {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "activity_climb_outputs")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub activity_id: i32,
        pub detector_version: i32,
        pub recording_sha256: String,
        pub object_ref: Json,
        pub climb_count: i32,
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
        pub fn matches(&self, recording_sha256: &str) -> bool {
            self.detector_version == DETECTOR_VERSION && self.recording_sha256 == recording_sha256
        }

        pub async fn for_activity(db: &impl ConnectionTrait, activity_id: i32) -> Result<Option<Self>, DbErr> {
            Entity::find_by_id(activity_id).one(db).await
        }

        pub async fn pending_tasks(db: &impl ConnectionTrait, after_id: i32) -> Result<Vec<ActivityTask>, DbErr> {
            let rows = PendingActivity::find_by_statement(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT a.id AS activity_id, a.user_id FROM activities a
                 WHERE a.id > $1 AND NOT EXISTS (
                    SELECT 1 FROM activity_climb_outputs o
                    WHERE o.activity_id = a.id AND o.detector_version = $2
                 ) ORDER BY a.id LIMIT 16",
                vec![after_id.into(), DETECTOR_VERSION.into()],
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

        pub(super) async fn save_output(
            db: &impl ConnectionTrait,
            activity_id: i32,
            climbs: &ClimbOutput,
            reference: &ObjectRef,
        ) -> Result<(), WorkflowError> {
            let object_ref =
                serde_json::to_value(reference).map_err(|error| WorkflowError::internal(error.to_string()))?;
            Entity::insert(ActiveModel {
                activity_id: Set(activity_id),
                detector_version: Set(climbs.detector_version),
                recording_sha256: Set(climbs.recording_sha256.clone()),
                object_ref: Set(object_ref),
                climb_count: Set(climbs.climbs.len() as i32),
            })
            .on_conflict(
                OnConflict::column(Column::ActivityId)
                    .update_columns([
                        Column::DetectorVersion,
                        Column::RecordingSha256,
                        Column::ObjectRef,
                        Column::ClimbCount,
                    ])
                    .to_owned(),
            )
            .exec_without_returning(db)
            .await?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support as fixtures;
    use std::collections::BTreeMap;

    #[tokio::test]
    async fn backfill_selects_only_missing_current_outputs_in_bounded_id_order() {
        let activity = fixtures::activity();
        let row: BTreeMap<String, Value> = [
            ("activity_id".into(), activity.id.into()),
            ("user_id".into(), activity.user_id.into()),
        ]
        .into();
        let db = fixtures::postgres_mock()
            .append_query_results([vec![row]])
            .into_connection();
        let tasks = output::Model::pending_tasks(&db, 17).await.unwrap();
        assert_eq!(
            tasks,
            vec![ActivityTask {
                priority: Priority::Background,
                ..fixtures::activity_task()
            }]
        );
        let sql = format!("{:?}", db.into_transaction_log());
        assert!(sql.contains("NOT EXISTS"));
        assert!(sql.contains("detector_version"));
        assert!(sql.contains("ORDER BY a.id LIMIT 16"));
    }
}
