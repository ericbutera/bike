//! Deterministic model fixtures shared by Rust unit and adapter tests.
//!
//! Override scenario fields with struct update syntax. Use `new_activity_import`
//! for database inserts so unrelated seeds never reuse the fixture primary key.
mod accounts;
mod analytics;
mod defaults;
pub use analytics::{
    analytics_user_state, fitness_day, new_fitness_day, new_segment_effort, segment_effort,
};
pub use defaults::{new_background_task, new_feature_flag, new_user_preferences};
mod heatmaps;
pub use heatmaps::heatmap_projection;
mod imports;
mod persistence;
pub use persistence::{insert, update};
mod schema;
mod segments;
pub use schema::table_with_timestamp_defaults;
pub use segments::segment;

pub use accounts::{connection, customer, new_connection, new_customer};
pub use imports::{archive_import_job, import_artifact};

use crate::entities::{activities, activity_imports};
use chrono::{DateTime, Utc};
use sea_orm::ActiveValue::NotSet;

pub fn timestamp() -> DateTime<Utc> {
    DateTime::from_timestamp(1788220800, 0).expect("fixed fixture timestamp")
}

pub fn activity_import() -> activity_imports::Model {
    let at = timestamp();
    activity_imports::Model {
        id: 18,
        user_id: 7,
        import_version: activity_imports::ACTIVITY_IMPORT_VERSION_CURRENT,
        source: "manual_upload".into(),
        format: "gpx".into(),
        status: "uploaded".into(),
        activity_id: None,
        archive_job_id: None,
        processing_stage: "raw_stored".into(),
        processing_error: None,
        processing_attempts: 0,
        processed_at: None,
        last_processing_event_at: Some(at),
        original_filename: "fixture.gpx".into(),
        storage_path: "activity-imports/fixture.gpx".into(),
        size_bytes: 123,
        mime_type: Some("application/gpx+xml".into()),
        created_at: at,
        updated_at: at,
    }
}

pub fn new_activity_import() -> activity_imports::ActiveModel {
    let mut active: activity_imports::ActiveModel = activity_import().into();
    active.id = NotSet;
    active
}

pub fn activity() -> activities::Model {
    let at = timestamp();
    activities::Model {
        id: 42,
        user_id: 7,
        activity_import_id: None,
        title: "Fixture ride".into(),
        sport: "ride".into(),
        source: "manual_upload".into(),
        source_correlation_id: None,
        original_filename: None,
        format: None,
        activity_type: "training".into(),
        started_at: at,
        ended_at: None,
        distance_meters: None,
        moving_time_seconds: None,
        total_time_seconds: None,
        elevation_gain_meters: None,
        elevation_loss_meters: None,
        average_speed_mps: None,
        max_speed_mps: None,
        average_heart_rate_bpm: None,
        max_heart_rate_bpm: None,
        average_cadence_rpm: None,
        max_cadence_rpm: None,
        calories: None,
        estimated_ftp_watts: None,
        heart_rate_zones_json: None,
        derived_data_json: None,
        created_at: at,
        updated_at: at,
    }
}

pub fn new_activity() -> activities::ActiveModel {
    let mut active: activities::ActiveModel = activity().into();
    active.id = NotSet;
    active
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, Schema};

    #[test]
    fn model_fixtures_share_a_stable_owner_and_clock() {
        let customer = customer();
        let connection = connection();
        let activity = activity();
        let import = activity_import();
        let artifact = import_artifact();
        let archive = archive_import_job();
        let state = analytics_user_state();
        let day = fitness_day();
        let effort = segment_effort();
        assert_eq!(activity.user_id, customer.id);
        assert_eq!(import.user_id, customer.id);
        assert_eq!(connection.user_id, customer.id);
        assert_eq!(artifact.user_id, customer.id);
        assert_eq!(artifact.activity_import_id, import.id);
        assert_eq!(archive.user_id, customer.id);
        assert_eq!(state.user_id, customer.id);
        assert_eq!(day.user_id, customer.id);
        assert_eq!(effort.user_id, customer.id);
        assert_eq!(activity.created_at, customer.created_at);
        assert_eq!(import.created_at, customer.created_at);
        assert_eq!(artifact.created_at, customer.created_at);
        assert_eq!(archive.created_at, customer.created_at);
        assert_eq!(state.created_at, customer.created_at);
        assert_eq!(day.created_at, customer.created_at);
        assert_eq!(effort.created_at, customer.created_at);
        assert_eq!(activity, super::activity());
        assert_eq!(import, activity_import());
    }

    #[tokio::test]
    async fn insert_fixtures_receive_distinct_database_ids() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute(
            &Schema::new(db.get_database_backend())
                .create_table_from_entity(activity_imports::Entity),
        )
        .await
        .unwrap();
        db.execute(
            &Schema::new(db.get_database_backend()).create_table_from_entity(activities::Entity),
        )
        .await
        .unwrap();
        let first = insert(&db, new_activity_import()).await;
        let second = insert(&db, new_activity_import()).await;
        assert_ne!(first.id, second.id);
        assert_eq!(first.user_id, second.user_id);
        assert_eq!(first.original_filename, second.original_filename);
        let first_activity = insert(&db, new_activity()).await;
        let second_activity = insert(&db, new_activity()).await;
        assert_ne!(first_activity.id, second_activity.id);
        assert_eq!(first_activity.user_id, second_activity.user_id);
    }

    #[tokio::test]
    async fn update_fixture_preserves_omitted_fields_and_explicitly_clears_values() {
        use sea_orm::Set;
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute(
            &Schema::new(db.get_database_backend()).create_table_from_entity(activities::Entity),
        )
        .await
        .unwrap();
        let original = insert(
            &db,
            activities::ActiveModel {
                estimated_ftp_watts: Set(Some(250)),
                ..new_activity()
            },
        )
        .await;
        let updated = update(
            &db,
            activities::ActiveModel {
                id: Set(original.id),
                title: Set("Updated fixture".into()),
                estimated_ftp_watts: Set(None),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(updated.id, original.id);
        assert_eq!(updated.title, "Updated fixture");
        assert_eq!(updated.estimated_ftp_watts, None);
        assert_eq!(updated.user_id, original.user_id);
        assert_eq!(updated.sport, original.sport);
        assert_eq!(updated.created_at, original.created_at);
        assert!(updated.updated_at > original.updated_at);
    }

    #[tokio::test]
    async fn database_timestamp_defaults_round_trip_through_version_filters() {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute(&table_with_timestamp_defaults(
            &db,
            activities::Entity,
            &["created_at", "updated_at"],
        ))
        .await
        .unwrap();
        let model = insert(
            &db,
            activities::ActiveModel {
                created_at: NotSet,
                updated_at: NotSet,
                ..new_activity()
            },
        )
        .await;
        let matching = activities::Entity::find_by_id(model.id)
            .filter(activities::Column::UpdatedAt.eq(model.updated_at))
            .one(&db)
            .await
            .unwrap();
        assert!(
            matching.is_some(),
            "database timestamps must match SQLx parameters"
        );
    }
}
