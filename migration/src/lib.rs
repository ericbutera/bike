#![allow(
    clippy::too_many_lines,
    reason = "migration DDL functions are append-only schema definitions"
)]

pub use sea_orm_migration::prelude::*;
mod m20260506_000001_create_activity_imports;
mod m20260506_000002_create_activities;
mod m20260506_000003_add_activity_derived_data;
mod m20260506_000004_create_segments;
mod m20260507_000005_create_user_preferences;
mod m20260507_000006_add_training_profile_fields;
mod m20260507_000007_add_analytics_cache_tables;
mod m20260507_000008_add_analytics_freshness_state;
mod m20260508_000009_create_activity_archive_import_jobs;
mod m20260512_000010_create_strava_connections;
mod m20260512_000011_create_activity_import_locks;
mod m20260512_000012_compact_activity_derived_data_json;
mod m20260512_000013_compact_segment_and_heart_rate_json;
mod m20260512_000014_add_activity_source_correlation_id;
mod m20260513_000015_create_integration_events;
mod m20260515_000001_map_feature_flag;
mod m20260520_000001_segment_summary_latest_activity;
mod m20260520_000002_activity_analytics_and_fitness_dirty_days;
mod m20260520_000003_rename_activity_analytics_top_10_count;
mod m20260520_000004_segment_builder_source;
mod m20260521_000001_segment_mode;
mod m20260521_000002_create_activity_training_analyses;
mod m20260521_000003_activity_training_analysis_focus_fields;
mod m20260521_000004_activity_training_analysis_decoupling;
mod m20260521_000005_user_preferences_xc_goal;
mod m20260522_000001_user_preferences_xc_goal_start_date;
mod m20260524_000001_user_preferences_xc_goal_backfill_status;
mod m20260527_000001_segment_starred;
mod m20260602_000001_create_garmin_iq_devices;
mod m20260622_000001_activity_is_race;
mod m20260622_000001_activity_type;
mod m20260622_000002_activity_import_processing_checkpoints;
mod m20260717_000001_add_oidc_identity_to_users;
mod m20260722_000001_user_preferences_xc_event_target_details;
mod m20260816_000001_create_activity_import_artifacts;
mod m20260816_000002_add_activity_import_version;
mod m20260816_000003_add_admin_activities_sort_index;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    #[expect(
        clippy::vec_init_then_push,
        reason = "migration registry keeps one migration per line for append-only review"
    )]
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        let mut migrations = kaleido::migrations::external_migrations();
        migrations.sort_by_key(|m| m.name().to_string());

        let mut locals: Vec<Box<dyn MigrationTrait>> = Vec::new();
        locals.push(Box::new(
            m20260506_000001_create_activity_imports::Migration,
        ));
        locals.push(Box::new(m20260506_000002_create_activities::Migration));
        locals.push(Box::new(
            m20260506_000003_add_activity_derived_data::Migration,
        ));
        locals.push(Box::new(m20260506_000004_create_segments::Migration));
        locals.push(Box::new(
            m20260507_000005_create_user_preferences::Migration,
        ));
        locals.push(Box::new(
            m20260507_000006_add_training_profile_fields::Migration,
        ));
        locals.push(Box::new(
            m20260507_000007_add_analytics_cache_tables::Migration,
        ));
        locals.push(Box::new(
            m20260507_000008_add_analytics_freshness_state::Migration,
        ));
        locals.push(Box::new(
            m20260508_000009_create_activity_archive_import_jobs::Migration,
        ));
        locals.push(Box::new(
            m20260512_000010_create_strava_connections::Migration,
        ));
        locals.push(Box::new(
            m20260512_000011_create_activity_import_locks::Migration,
        ));
        locals.push(Box::new(
            m20260512_000012_compact_activity_derived_data_json::Migration,
        ));
        locals.push(Box::new(
            m20260512_000013_compact_segment_and_heart_rate_json::Migration,
        ));
        locals.push(Box::new(
            m20260512_000014_add_activity_source_correlation_id::Migration,
        ));
        locals.push(Box::new(
            m20260513_000015_create_integration_events::Migration,
        ));
        locals.push(Box::new(m20260515_000001_map_feature_flag::Migration));
        locals.push(Box::new(
            m20260520_000001_segment_summary_latest_activity::Migration,
        ));
        locals.push(Box::new(
            m20260520_000002_activity_analytics_and_fitness_dirty_days::Migration,
        ));
        locals.push(Box::new(
            m20260520_000003_rename_activity_analytics_top_10_count::Migration,
        ));
        locals.push(Box::new(m20260520_000004_segment_builder_source::Migration));
        locals.push(Box::new(m20260521_000001_segment_mode::Migration));
        locals.push(Box::new(
            m20260521_000002_create_activity_training_analyses::Migration,
        ));
        locals.push(Box::new(
            m20260521_000003_activity_training_analysis_focus_fields::Migration,
        ));
        locals.push(Box::new(
            m20260521_000004_activity_training_analysis_decoupling::Migration,
        ));
        locals.push(Box::new(
            m20260521_000005_user_preferences_xc_goal::Migration,
        ));
        locals.push(Box::new(
            m20260522_000001_user_preferences_xc_goal_start_date::Migration,
        ));
        locals.push(Box::new(
            m20260524_000001_user_preferences_xc_goal_backfill_status::Migration,
        ));
        locals.push(Box::new(m20260527_000001_segment_starred::Migration));
        locals.push(Box::new(
            m20260602_000001_create_garmin_iq_devices::Migration,
        ));
        locals.push(Box::new(m20260622_000001_activity_is_race::Migration));
        locals.push(Box::new(m20260622_000001_activity_type::Migration));
        locals.push(Box::new(
            m20260622_000002_activity_import_processing_checkpoints::Migration,
        ));
        locals.push(Box::new(
            m20260717_000001_add_oidc_identity_to_users::Migration,
        ));
        locals.push(Box::new(
            m20260722_000001_user_preferences_xc_event_target_details::Migration,
        ));
        locals.push(Box::new(
            m20260816_000001_create_activity_import_artifacts::Migration,
        ));
        locals.push(Box::new(
            m20260816_000002_add_activity_import_version::Migration,
        ));
        locals.push(Box::new(
            m20260816_000003_add_admin_activities_sort_index::Migration,
        ));
        locals.sort_by_key(|m| m.name().to_string());

        migrations.extend(locals);
        migrations
    }
}
