#![allow(
    clippy::too_many_lines,
    reason = "migration DDL functions are append-only schema definitions"
)]

pub use sea_orm_migration::prelude::*;
mod kaleido_migrations;
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
mod m20260912_000001_create_provider_rate_limit_buckets;
mod m20260928_000001_activity_maps_feature_flag;
mod m20260929_000001_strava_gateway_receipts;
mod m20260929_000002_strava_gateway_revocations;
mod m20260929_000003_strava_gateway_binding_leases;
mod m20261001_000001_create_oauth_states;
mod m20261003_000001_heatmaps_feature_flag;
mod m20261003_000002_heatmap_projections;
mod m20261004_000001_synthetic_scenarios;
mod m20261004_000002_heatmap_projection_speed_filter;
mod m20261005_000001_heatmap_projection_virtual_ride_filter;
mod m20261005_000002_strava_gateway_integration_events;
mod m20261006_000001_heatmap_projection_recording_policy;
mod m20261006_000002_heatmap_cycling_sources;
mod m20261006_000003_activity_recording_environment;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        let mut migrations = kaleido_migrations::bike_platform_migrations();
        migrations.sort_by_key(|m| m.name().to_string());

        let mut locals: Vec<Box<dyn MigrationTrait>> = vec![
            Box::new(m20260506_000001_create_activity_imports::Migration),
            Box::new(m20260506_000002_create_activities::Migration),
            Box::new(m20260506_000003_add_activity_derived_data::Migration),
            Box::new(m20260506_000004_create_segments::Migration),
            Box::new(m20260507_000005_create_user_preferences::Migration),
            Box::new(m20260507_000006_add_training_profile_fields::Migration),
            Box::new(m20260507_000007_add_analytics_cache_tables::Migration),
            Box::new(m20260507_000008_add_analytics_freshness_state::Migration),
            Box::new(m20260508_000009_create_activity_archive_import_jobs::Migration),
            Box::new(m20260512_000010_create_strava_connections::Migration),
            Box::new(m20260512_000011_create_activity_import_locks::Migration),
            Box::new(m20260512_000012_compact_activity_derived_data_json::Migration),
            Box::new(m20260512_000013_compact_segment_and_heart_rate_json::Migration),
            Box::new(m20260512_000014_add_activity_source_correlation_id::Migration),
            Box::new(m20260513_000015_create_integration_events::Migration),
            Box::new(m20260515_000001_map_feature_flag::Migration),
            Box::new(m20260520_000001_segment_summary_latest_activity::Migration),
            Box::new(m20260520_000002_activity_analytics_and_fitness_dirty_days::Migration),
            Box::new(m20260520_000003_rename_activity_analytics_top_10_count::Migration),
            Box::new(m20260520_000004_segment_builder_source::Migration),
            Box::new(m20260521_000001_segment_mode::Migration),
            Box::new(m20260521_000002_create_activity_training_analyses::Migration),
            Box::new(m20260521_000003_activity_training_analysis_focus_fields::Migration),
            Box::new(m20260521_000004_activity_training_analysis_decoupling::Migration),
            Box::new(m20260521_000005_user_preferences_xc_goal::Migration),
            Box::new(m20260522_000001_user_preferences_xc_goal_start_date::Migration),
            Box::new(m20260524_000001_user_preferences_xc_goal_backfill_status::Migration),
            Box::new(m20260527_000001_segment_starred::Migration),
            Box::new(m20260602_000001_create_garmin_iq_devices::Migration),
            Box::new(m20260622_000001_activity_is_race::Migration),
            Box::new(m20260622_000001_activity_type::Migration),
            Box::new(m20260622_000002_activity_import_processing_checkpoints::Migration),
            Box::new(m20260717_000001_add_oidc_identity_to_users::Migration),
            Box::new(m20260722_000001_user_preferences_xc_event_target_details::Migration),
            Box::new(m20260816_000001_create_activity_import_artifacts::Migration),
            Box::new(m20260816_000002_add_activity_import_version::Migration),
            Box::new(m20260816_000003_add_admin_activities_sort_index::Migration),
            Box::new(m20260912_000001_create_provider_rate_limit_buckets::Migration),
            Box::new(m20260928_000001_activity_maps_feature_flag::Migration),
            Box::new(m20260929_000001_strava_gateway_receipts::Migration),
            Box::new(m20260929_000002_strava_gateway_revocations::Migration),
            Box::new(m20260929_000003_strava_gateway_binding_leases::Migration),
            Box::new(m20261001_000001_create_oauth_states::Migration),
            Box::new(m20261003_000001_heatmaps_feature_flag::Migration),
            Box::new(m20261003_000002_heatmap_projections::Migration),
            Box::new(m20261004_000001_synthetic_scenarios::Migration),
            Box::new(m20261004_000002_heatmap_projection_speed_filter::Migration),
            Box::new(m20261005_000001_heatmap_projection_virtual_ride_filter::Migration),
            Box::new(m20261005_000002_strava_gateway_integration_events::Migration),
            Box::new(m20261006_000001_heatmap_projection_recording_policy::Migration),
            Box::new(m20261006_000002_heatmap_cycling_sources::Migration),
            Box::new(m20261006_000003_activity_recording_environment::Migration),
        ];
        locals.sort_by_key(|m| m.name().to_string());

        migrations.extend(locals);
        migrations
    }
}

#[cfg(test)]
mod tests {
    use super::{Migrator, MigratorTrait};

    #[test]
    fn retains_all_kaleido_migration_ids_in_their_original_order() {
        let expected = [
            "m20220101_000001_users",
            "m20220101_000002_refresh_tokens",
            "m20260107_120000_create_cooldowns",
            "m20260122_120000_background_tasks",
            "m20260124_120500_api_clients",
            "m20260125_212110_create_feature_flags",
            "m20260210_000001_auth_events",
            "m20260312_000000_background_tasks_result",
            "m20260724_000001_remove_oauth_feature_flag",
            "m20260724_000002_rename_oauth_subject_column",
            "m20260928_000001_add_user_disabled",
        ];
        let actual = Migrator::migrations()
            .into_iter()
            .map(|migration| migration.name().to_string())
            .filter(|name| expected.contains(&name.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(actual, expected);
    }
}
