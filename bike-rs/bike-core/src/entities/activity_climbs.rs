use crate::{
    activity_climbs::{Climb, PreparedClimb, PreparedOutput, DETECTOR_VERSION},
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
    pub vertical_rate_meters_per_hour: Option<f64>,
    pub average_speed_mps: Option<f64>,
    pub average_heart_rate_bpm: Option<f64>,
    pub peak_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<f64>,
    pub average_power_watts: Option<f64>,
    pub heart_rate_recovery_30_seconds_bpm: Option<i32>,
    pub heart_rate_recovery_60_seconds_bpm: Option<i32>,
    pub seconds_to_drop_10_bpm: Option<i32>,
    pub seconds_to_drop_15_bpm: Option<i32>,
    pub summit_immediately_enters_descent: Option<bool>,
    pub is_first_half: Option<bool>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

impl ActiveModel {
    fn from_climb(activity_id: i32, prepared: &PreparedClimb) -> Self {
        let climb = &prepared.climb;
        let facts = &prepared.facts;
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
            vertical_rate_meters_per_hour: Set(Some(facts.vertical_rate_meters_per_hour)),
            average_speed_mps: Set(facts.average_speed_mps),
            average_heart_rate_bpm: Set(facts.average_heart_rate_bpm),
            peak_heart_rate_bpm: Set(facts.peak_heart_rate_bpm),
            average_cadence_rpm: Set(facts.average_cadence_rpm),
            average_power_watts: Set(facts.average_power_watts),
            heart_rate_recovery_30_seconds_bpm: Set(facts.heart_rate_recovery_30_seconds_bpm),
            heart_rate_recovery_60_seconds_bpm: Set(facts.heart_rate_recovery_60_seconds_bpm),
            seconds_to_drop_10_bpm: Set(facts.seconds_to_drop_10_bpm),
            seconds_to_drop_15_bpm: Set(facts.seconds_to_drop_15_bpm),
            summit_immediately_enters_descent: Set(Some(facts.summit_immediately_enters_descent)),
            is_first_half: Set(Some(facts.is_first_half)),
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

    pub(crate) async fn replace_output(
        db: &DatabaseConnection,
        source: &super::activities::Model,
        climbs: &PreparedOutput,
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
        pub object_path: String,
        pub object_sha256: String,
        pub object_bytes: i64,
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
        pub fn object_reference(&self) -> Result<ObjectRef, WorkflowError> {
            Ok(ObjectRef {
                path: self.object_path.clone(),
                sha256: self.object_sha256.clone(),
                bytes: usize::try_from(self.object_bytes)
                    .map_err(|error| WorkflowError::invalid_data(error.to_string()))?,
            })
        }

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
            climbs: &PreparedOutput,
            reference: &ObjectRef,
        ) -> Result<(), WorkflowError> {
            let object_bytes =
                i64::try_from(reference.bytes).map_err(|error| WorkflowError::invalid_data(error.to_string()))?;
            Entity::insert(ActiveModel {
                activity_id: Set(activity_id),
                detector_version: Set(climbs.detector_version),
                recording_sha256: Set(climbs.recording_sha256.clone()),
                object_path: Set(reference.path.clone()),
                object_sha256: Set(reference.sha256.clone()),
                object_bytes: Set(object_bytes),
                climb_count: Set(climbs.climbs.len() as i32),
            })
            .on_conflict(
                OnConflict::column(Column::ActivityId)
                    .update_columns([
                        Column::DetectorVersion,
                        Column::RecordingSha256,
                        Column::ObjectPath,
                        Column::ObjectSha256,
                        Column::ObjectBytes,
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

    #[test]
    fn climb_rows_retain_each_detected_report_fact_in_its_original_units() {
        use sea_orm::TryIntoModel;
        let report = crate::services::reports::ClimbResponse {
            activity_id: 42,
            activity_title: "Hill repeats".into(),
            climb_number: 2,
            start_seconds: 10,
            summit_seconds: 130,
            duration_seconds: 120,
            distance_meters: 520.0,
            gain_meters: 26.0,
            average_grade_percent: Some(5.0),
            vertical_rate_meters_per_hour: 780.0,
            average_speed_mps: Some(4.33),
            average_heart_rate_bpm: Some(140.8),
            peak_heart_rate_bpm: Some(160),
            average_cadence_rpm: Some(81.0),
            average_power_watts: Some(220.0),
            heart_rate_recovery_30_seconds_bpm: Some(6),
            heart_rate_recovery_60_seconds_bpm: Some(18),
            seconds_to_drop_10_bpm: Some(40),
            seconds_to_drop_15_bpm: Some(60),
            summit_immediately_enters_descent: true,
            first_or_second_half: "first".into(),
        };
        let prepared = PreparedClimb::from_detection(report, &[]);
        let row = ActiveModel::from_climb(42, &prepared).try_into_model().unwrap();
        assert_eq!(row.climb_number, 2);
        assert_eq!(row.start_elapsed_seconds, 10);
        assert_eq!(row.end_elapsed_seconds, 130);
        assert_eq!(row.vertical_rate_meters_per_hour, Some(780.0));
        assert_eq!(row.average_speed_mps, Some(4.33));
        assert_eq!(row.average_heart_rate_bpm, Some(140.8));
        assert_eq!(row.peak_heart_rate_bpm, Some(160));
        assert_eq!(row.average_cadence_rpm, Some(81.0));
        assert_eq!(row.average_power_watts, Some(220.0));
        assert_eq!(row.heart_rate_recovery_30_seconds_bpm, Some(6));
        assert_eq!(row.heart_rate_recovery_60_seconds_bpm, Some(18));
        assert_eq!(row.seconds_to_drop_10_bpm, Some(40));
        assert_eq!(row.seconds_to_drop_15_bpm, Some(60));
        assert_eq!(row.summit_immediately_enters_descent, Some(true));
        assert_eq!(row.is_first_half, Some(true));
    }

    #[test]
    fn typed_climb_reference_preserves_path_checksum_and_byte_count() {
        let output = fixtures::activity_climb_output();
        let reference = output.object_reference().unwrap();
        assert_eq!(reference.path, output.object_path);
        assert_eq!(reference.sha256, output.object_sha256);
        assert_eq!(reference.bytes as i64, output.object_bytes);
        let invalid = output::Model {
            object_bytes: -1,
            ..output
        };
        assert!(invalid.object_reference().is_err());
    }
}
