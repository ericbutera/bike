use crate::activity_details::ActivityRoutePoint;
use crate::analytics::{mark_segment_activity_changes, mark_user_activity_change, mark_user_fitness_dirty};
use crate::dedupe::{activity_duplicate_candidate_key, activity_models_match_for_dedupe};
use crate::entities::{activities, activity_analytics, activity_imports, activity_training_analyses, segment_efforts};
use crate::jobs::JobQueue as TaskQueue;
use crate::segment_support::{clear_segment_efforts_for_activity, replace_segment_efforts_for_activity};
use crate::workflow_error::WorkflowError as AppError;
use chrono::Utc;
use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionSession, TransactionTrait};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub async fn update_activity_details(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    activity: activities::Model,
    title: Option<String>,
    activity_type: Option<crate::activity_type::ActivityType>,
) -> Result<activities::Model, AppError> {
    let updated = activity.update_details(db, title, activity_type).await?;
    tasks.prepare_heatmap(updated.user_id, updated.id).await?;
    Ok(updated)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateActivityCleanupSummary {
    pub duplicate_group_count: usize,
    pub deleted_activity_count: usize,
    pub retained_activity_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DuplicateActivityCleanupPlan {
    duplicate_group_count: usize,
    duplicate_activity_ids: Vec<i32>,
    retained_activity_ids: Vec<i32>,
}

#[derive(Debug, Clone)]
struct DuplicateActivityCandidate {
    activity: activities::Model,
    route_point_count: usize,
    format_rank: i32,
}

pub async fn refresh_activity_derived_state_without_cache_rebuilds<C>(
    db: &C,
    user_id: i32,
    activity_id: i32,
    route_points: &[ActivityRoutePoint],
) -> Result<Vec<i32>, AppError>
where
    C: ConnectionTrait + TransactionTrait,
{
    let mut affected_segment_ids = load_segment_ids_for_activity(db, activity_id).await?;

    replace_segment_efforts_for_activity(db, user_id, activity_id, route_points).await?;

    affected_segment_ids.extend(load_segment_ids_for_activity(db, activity_id).await?);
    affected_segment_ids.sort_unstable();
    affected_segment_ids.dedup();

    Ok(affected_segment_ids)
}

pub async fn delete_activity_with_derived_state<C: ConnectionTrait + TransactionTrait>(
    db: &C,
    uploads_dir: &str,
    user_id: i32,
    activity: activities::Model,
) -> Result<Vec<i32>, AppError> {
    let txn = db.begin().await?;
    let deleted = delete_activity_records(&txn, user_id, activity).await?;
    txn.commit().await?;
    if let Some(source) = deleted.source {
        remove_upload_file(Path::new(uploads_dir).join(source)).await;
    }
    Ok(deleted.segment_ids)
}

pub struct DeletedActivity {
    pub segment_ids: Vec<i32>,
    pub source: Option<String>,
}

#[tracing::instrument(name = "activity.delete.records", skip_all, fields(user_id, activity_id = activity.id))]
pub async fn delete_activity_records(
    db: &impl ConnectionTrait,
    user_id: i32,
    activity: activities::Model,
) -> Result<DeletedActivity, AppError> {
    let mut source = None;
    let affected_segment_ids = load_segment_ids_for_activity(db, activity.id).await?;

    clear_segment_efforts_for_activity(db, user_id, activity.id).await?;

    activity_analytics::Entity::delete_by_activity_id(db, activity.id).await?;
    activity_training_analyses::Entity::delete_by_activity_id(db, activity.id).await?;
    activities::Entity::delete_by_user(db, activity.id, user_id).await?;

    if let Some(activity_import_id) = activity.activity_import_id {
        let remaining_references =
            activities::Entity::count_by_import_for_user(db, activity_import_id, user_id).await?;

        if remaining_references == 0 {
            let linked_import = activity_imports::Entity::find_owned(db, user_id, activity_import_id).await?;

            if let Some(linked_import) = linked_import {
                source = Some(linked_import.storage_path);

                activity_imports::Entity::delete_by_user(db, activity_import_id, user_id).await?;
            }
        }
    }

    Ok(DeletedActivity {
        segment_ids: affected_segment_ids,
        source,
    })
}

pub async fn cleanup_duplicate_activities_for_user(
    db: &DatabaseConnection,
    uploads_dir: &str,
    tasks: &TaskQueue,
    user_id: i32,
) -> Result<DuplicateActivityCleanupSummary, AppError> {
    let activities = activities::Model::for_user(db, user_id).await?;
    let cleanup_plan = plan_duplicate_activity_cleanup(activities);

    if cleanup_plan.duplicate_activity_ids.is_empty() {
        return Ok(DuplicateActivityCleanupSummary {
            duplicate_group_count: 0,
            deleted_activity_count: 0,
            retained_activity_count: 0,
        });
    }

    let activities_by_id =
        activities::Model::list_owned_by_ids(db, user_id, cleanup_plan.duplicate_activity_ids.iter().copied())
            .await?
            .into_iter()
            .map(|activity| (activity.id, activity))
            .collect::<HashMap<_, _>>();
    let mut affected_segment_ids = Vec::new();
    let mut fitness_dirty_from_day: Option<chrono::NaiveDate> = None;

    for activity_id in &cleanup_plan.duplicate_activity_ids {
        let Some(activity) = activities_by_id.get(activity_id).cloned() else {
            tracing::warn!(
                user_id,
                activity_id,
                "skipping duplicate cleanup because the activity was already removed"
            );
            continue;
        };

        fitness_dirty_from_day = Some(match fitness_dirty_from_day {
            Some(current) => current.min(activity.started_at.date_naive()),
            None => activity.started_at.date_naive(),
        });
        affected_segment_ids.extend(delete_activity_with_derived_state(db, uploads_dir, user_id, activity).await?);
    }

    let changed_at = Utc::now();
    if let Some(dirty_from_day) = fitness_dirty_from_day {
        mark_user_fitness_dirty(db, user_id, dirty_from_day, changed_at).await?;
    } else {
        mark_user_activity_change(db, user_id, changed_at).await?;
    }
    mark_segment_activity_changes(db, &affected_segment_ids, changed_at).await?;
    tasks.rebuild_fitness_freshness(user_id).await?;
    tasks.rebuild_segment_analytics(affected_segment_ids).await?;

    Ok(DuplicateActivityCleanupSummary {
        duplicate_group_count: cleanup_plan.duplicate_group_count,
        deleted_activity_count: cleanup_plan.duplicate_activity_ids.len(),
        retained_activity_count: cleanup_plan.retained_activity_ids.len(),
    })
}

pub async fn load_segment_ids_for_activity<C>(db: &C, activity_id: i32) -> Result<Vec<i32>, AppError>
where
    C: ConnectionTrait,
{
    let mut segment_ids = segment_efforts::Model::segment_ids_for_activity(db, activity_id).await?;
    segment_ids.sort_unstable();
    segment_ids.dedup();

    Ok(segment_ids)
}

fn plan_duplicate_activity_cleanup(activities: Vec<activities::Model>) -> DuplicateActivityCleanupPlan {
    let mut activities_by_key = HashMap::<String, Vec<DuplicateActivityCandidate>>::new();

    for activity in activities {
        let route_point_count =
            crate::activity_details::deserialize_derived_activity_data(activity.derived_data_json.as_ref())
                .route_points
                .len();
        let key = activity_duplicate_candidate_key(activity.started_at, &activity.sport);

        activities_by_key
            .entry(key)
            .or_default()
            .push(DuplicateActivityCandidate {
                format_rank: activity_cleanup_format_rank(&activity),
                route_point_count,
                activity,
            });
    }

    let mut grouped_candidates = activities_by_key.into_iter().collect::<Vec<_>>();
    grouped_candidates.sort_by(|left, right| left.0.cmp(&right.0));

    let mut duplicate_activity_ids = Vec::new();
    let mut retained_activity_ids = Vec::new();
    let mut duplicate_group_count = 0usize;

    for (_, mut candidates) in grouped_candidates {
        if candidates.len() < 2 {
            continue;
        }

        candidates.sort_by(compare_duplicate_activity_candidates);

        let mut keepers = Vec::<(DuplicateActivityCandidate, bool)>::new();

        for candidate in candidates {
            if let Some((_, has_duplicates)) = keepers
                .iter_mut()
                .find(|(keeper, _)| activity_models_match_for_dedupe(&keeper.activity, &candidate.activity))
            {
                *has_duplicates = true;
                duplicate_activity_ids.push(candidate.activity.id);
            } else {
                keepers.push((candidate, false));
            }
        }

        for (keeper, has_duplicates) in keepers {
            if has_duplicates {
                duplicate_group_count += 1;
                retained_activity_ids.push(keeper.activity.id);
            }
        }
    }

    duplicate_activity_ids.sort_unstable();
    retained_activity_ids.sort_unstable();

    DuplicateActivityCleanupPlan {
        duplicate_group_count,
        duplicate_activity_ids,
        retained_activity_ids,
    }
}

fn compare_duplicate_activity_candidates(
    left: &DuplicateActivityCandidate,
    right: &DuplicateActivityCandidate,
) -> Ordering {
    right
        .format_rank
        .cmp(&left.format_rank)
        .then_with(|| right.route_point_count.cmp(&left.route_point_count))
        .then_with(|| left.activity.created_at.cmp(&right.activity.created_at))
        .then_with(|| left.activity.id.cmp(&right.activity.id))
}

fn activity_cleanup_format_rank(activity: &activities::Model) -> i32 {
    let format = activity.format.as_deref().or_else(|| {
        activity
            .original_filename
            .as_deref()
            .and_then(|filename| Path::new(filename).extension().and_then(|value| value.to_str()))
    });

    match format {
        Some(value) if value.eq_ignore_ascii_case("fit") => 3,
        Some(value) if value.eq_ignore_ascii_case("tcx") => 2,
        Some(value) if value.eq_ignore_ascii_case("gpx") => 1,
        _ => 0,
    }
}

async fn remove_upload_file(path: PathBuf) {
    if let Err(error) = tokio::fs::remove_file(&path).await {
        if error.kind() != ErrorKind::NotFound {
            tracing::warn!(
                error = ?error,
                path = %path.display(),
                "failed to remove upload file for deleted activity"
            );
        }
    }
}

#[cfg(test)]
mod tests {

    mod deletion;

    use super::*;
    use crate::activity_details::{serialize_derived_activity_data, ActivityDerivedData};
    use chrono::{DateTime, Duration};

    fn make_route_point(elapsed_seconds: i32, latitude: f64, longitude: f64) -> ActivityRoutePoint {
        ActivityRoutePoint {
            elapsed_seconds,
            latitude,
            longitude,
            distance_meters: Some(elapsed_seconds as f64 * 10.0),
            elevation_meters: Some(100.0),
            speed_mps: Some(8.0),
            heart_rate_bpm: Some(140),
            cadence_rpm: Some(88),
            power_watts: None,
        }
    }

    fn make_activity(
        id: i32,
        created_at: chrono::DateTime<Utc>,
        format: Option<&str>,
        original_filename: Option<&str>,
        route_points: Vec<ActivityRoutePoint>,
    ) -> activities::Model {
        activities::Model {
            id,
            user_id: 42,
            activity_import_id: Some(id + 1000),
            title: format!("Activity {id}"),
            original_filename: original_filename.map(str::to_string),
            format: format.map(str::to_string),
            started_at: DateTime::parse_from_rfc3339("2026-05-11T13:23:17Z")
                .expect("started_at")
                .with_timezone(&Utc),
            distance_meters: Some(122_768.0),
            moving_time_seconds: Some(27_011),
            total_time_seconds: Some(27_012),
            elevation_gain_meters: Some(500.0),
            elevation_loss_meters: Some(500.0),
            average_speed_mps: Some(7.5),
            max_speed_mps: Some(14.0),
            average_heart_rate_bpm: Some(140),
            max_heart_rate_bpm: Some(170),
            average_cadence_rpm: Some(86),
            max_cadence_rpm: Some(102),
            calories: Some(900),
            derived_data_json: Some(
                serialize_derived_activity_data(&ActivityDerivedData {
                    recording: Default::default(),
                    laps: Vec::new(),
                    chart_points: Vec::new(),
                    route_points,
                })
                .expect("serialize derived data"),
            ),
            created_at,
            updated_at: created_at,
            ..crate::test_support::activity()
        }
    }

    #[test]
    fn plans_duplicate_cleanup_prefers_fit_over_tcx() {
        let created_at = Utc::now();
        let route = vec![
            make_route_point(0, 44.7500, -85.6200),
            make_route_point(1800, 44.7200, -85.4200),
            make_route_point(3600, 44.7500, -85.6200),
        ];
        let fit_activity = make_activity(
            10,
            created_at + Duration::minutes(5),
            Some("fit"),
            Some("ride.fit"),
            route.clone(),
        );
        let tcx_activity = make_activity(11, created_at, Some("tcx"), Some("ride.tcx"), route);

        let plan = plan_duplicate_activity_cleanup(vec![fit_activity, tcx_activity]);

        assert_eq!(plan.duplicate_group_count, 1);
        assert_eq!(plan.retained_activity_ids, vec![10]);
        assert_eq!(plan.duplicate_activity_ids, vec![11]);
    }

    #[test]
    fn plans_duplicate_cleanup_skips_same_base_key_with_different_routes() {
        let created_at = Utc::now();
        let northern_route = vec![
            make_route_point(0, 45.1000, -122.1000),
            make_route_point(1800, 45.1200, -122.1200),
            make_route_point(3600, 45.1400, -122.1400),
        ];
        let southern_route = vec![
            make_route_point(0, 40.1000, -120.1000),
            make_route_point(1800, 40.1200, -120.1200),
            make_route_point(3600, 40.1400, -120.1400),
        ];

        let first_activity = make_activity(20, created_at, Some("fit"), Some("first.fit"), northern_route);
        let second_activity = make_activity(
            21,
            created_at + Duration::minutes(2),
            Some("fit"),
            Some("second.fit"),
            southern_route,
        );

        let plan = plan_duplicate_activity_cleanup(vec![first_activity, second_activity]);

        assert_eq!(plan.duplicate_group_count, 0);
        assert!(plan.retained_activity_ids.is_empty());
        assert!(plan.duplicate_activity_ids.is_empty());
    }

    #[test]
    fn plans_duplicate_cleanup_for_trimmed_tcx_variant() {
        let created_at = Utc::now();
        let full_route = vec![
            make_route_point(0, 44.7539, -85.6290),
            make_route_point(1800, 44.7600, -85.6000),
            make_route_point(3600, 44.7420, -85.5109),
        ];
        let trimmed_route = vec![
            make_route_point(295, 44.7552, -85.6176),
            make_route_point(1800, 44.7600, -85.6000),
            make_route_point(3300, 44.7414, -85.5091),
        ];

        let mut fit_activity = make_activity(
            1039,
            created_at + Duration::minutes(5),
            Some("fit"),
            Some("22742351729_ACTIVITY.fit"),
            full_route,
        );
        fit_activity.distance_meters = Some(28_396.91);
        fit_activity.moving_time_seconds = Some(11_107);
        fit_activity.total_time_seconds = Some(12_923);

        let mut tcx_activity = make_activity(
            1129,
            created_at,
            Some("tcx"),
            Some("MSB_cleanup_18351119858.tcx"),
            trimmed_route,
        );
        tcx_activity.distance_meters = Some(28_396.90);
        tcx_activity.moving_time_seconds = Some(12_925);
        tcx_activity.total_time_seconds = Some(12_925);

        let plan = plan_duplicate_activity_cleanup(vec![fit_activity, tcx_activity]);

        assert_eq!(plan.duplicate_group_count, 1);
        assert_eq!(plan.retained_activity_ids, vec![1039]);
        assert_eq!(plan.duplicate_activity_ids, vec![1129]);
    }
}
