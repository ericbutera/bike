use crate::analytics::{rebuild_fitness_freshness_cache, FATIGUE_WINDOW_DAYS, FITNESS_WINDOW_DAYS};
use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::{analytics_user_states, fitness_freshness_daily};
use crate::storage::AppStorage;
use axum::extract::{Query, State};
use axum::Json;
use chrono::{Duration, NaiveDate, Utc};
use kaleido::auth::UserContext;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams)]
pub struct FitnessQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FitnessFreshnessPoint {
    pub date: String,
    pub training_load: f64,
    pub fitness: f64,
    pub fatigue: f64,
    pub form: f64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FitnessFreshnessResponse {
    pub start_date: String,
    pub end_date: String,
    pub fitness_window_days: i32,
    pub fatigue_window_days: i32,
    pub points: Vec<FitnessFreshnessPoint>,
}

#[utoipa::path(
    get,
    path = "/api/fitness",
    params(FitnessQuery),
    responses(
        (status = 200, description = "Daily fitness, fatigue, and form data for the authenticated user", body = FitnessFreshnessResponse),
        (status = 400, description = "Invalid query parameters", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "fitness",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_fitness_freshness(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Query(query): Query<FitnessQuery>,
) -> Result<Json<FitnessFreshnessResponse>, AppError> {
    let requested_start_date = query.start_date.as_deref().map(parse_date).transpose()?;
    let end_date = query
        .end_date
        .as_deref()
        .map(parse_date)
        .transpose()?
        .unwrap_or_else(|| Utc::now().date_naive());

    if requested_start_date.is_some_and(|start_date| start_date > end_date) {
        return Err(AppError::validation_field(
            "start_date",
            "Start date must be on or before end date",
        ));
    }

    let cached_rows =
        load_cached_rows_for_request(&state.db, user.id, requested_start_date, end_date).await?;

    let start_date = requested_start_date
        .or_else(|| cached_rows.first().map(|row| row.day))
        .unwrap_or(end_date);
    let points = build_points_from_cached_rows(&cached_rows, start_date, end_date);

    Ok(Json(FitnessFreshnessResponse {
        start_date: start_date.format("%Y-%m-%d").to_string(),
        end_date: end_date.format("%Y-%m-%d").to_string(),
        fitness_window_days: FITNESS_WINDOW_DAYS as i32,
        fatigue_window_days: FATIGUE_WINDOW_DAYS as i32,
        points,
    }))
}

async fn load_cached_rows_for_request(
    db: &DatabaseConnection,
    user_id: i32,
    requested_start_date: Option<NaiveDate>,
    end_date: NaiveDate,
) -> Result<Vec<fitness_freshness_daily::Model>, AppError> {
    let cached_rows = load_cached_rows(db, user_id, requested_start_date, end_date).await?;
    let freshness_state = analytics_user_states::Entity::find_by_id(user_id)
        .one(db)
        .await?;

    if cache_is_usable(
        &cached_rows,
        freshness_state.as_ref(),
        requested_start_date,
        end_date,
    ) {
        return Ok(cached_rows);
    }

    rebuild_fitness_freshness_cache(db, user_id).await?;
    load_cached_rows(db, user_id, requested_start_date, end_date)
        .await
        .map_err(Into::into)
}

async fn load_cached_rows(
    db: &DatabaseConnection,
    user_id: i32,
    requested_start_date: Option<NaiveDate>,
    end_date: NaiveDate,
) -> Result<Vec<fitness_freshness_daily::Model>, sea_orm::DbErr> {
    let mut cached_rows_query = fitness_freshness_daily::Entity::find()
        .filter(fitness_freshness_daily::Column::UserId.eq(user_id))
        .filter(fitness_freshness_daily::Column::Day.lte(end_date))
        .order_by_asc(fitness_freshness_daily::Column::Day);

    if let Some(start_date) = cached_row_query_start_date(requested_start_date) {
        cached_rows_query =
            cached_rows_query.filter(fitness_freshness_daily::Column::Day.gte(start_date));
    }

    cached_rows_query.all(db).await
}

fn cache_is_usable(
    rows: &[fitness_freshness_daily::Model],
    freshness_state: Option<&analytics_user_states::Model>,
    requested_start_date: Option<NaiveDate>,
    end_date: NaiveDate,
) -> bool {
    if rows.is_empty() {
        return false;
    }

    if requested_start_date.is_some_and(|start_date| start_date < rows[0].day) {
        return false;
    }

    if rows
        .windows(2)
        .any(|window| window[1].day != window[0].day + Duration::days(1))
    {
        return false;
    }

    !freshness_state
        .and_then(|state| state.fitness_dirty_from_day)
        .is_some_and(|dirty_from_day| dirty_from_day <= end_date)
}

fn build_points_from_cached_rows(
    rows: &[fitness_freshness_daily::Model],
    requested_start_date: NaiveDate,
    end_date: NaiveDate,
) -> Vec<FitnessFreshnessPoint> {
    let mut points = rows
        .iter()
        .filter(|row| row.day >= requested_start_date)
        .map(|row| FitnessFreshnessPoint {
            date: row.day.format("%Y-%m-%d").to_string(),
            training_load: round_metric(row.training_load),
            fitness: round_metric(row.fitness),
            fatigue: round_metric(row.fatigue),
            form: round_metric(row.form),
        })
        .collect::<Vec<_>>();

    let Some(last_row) = rows.last() else {
        return points;
    };

    let mut current_date = last_row.day;
    let mut fitness = last_row.fitness;
    let mut fatigue = last_row.fatigue;

    while current_date < end_date {
        current_date += Duration::days(1);
        fitness += (0.0 - fitness) / FITNESS_WINDOW_DAYS;
        fatigue += (0.0 - fatigue) / FATIGUE_WINDOW_DAYS;

        if current_date >= requested_start_date {
            points.push(FitnessFreshnessPoint {
                date: current_date.format("%Y-%m-%d").to_string(),
                training_load: 0.0,
                fitness: round_metric(fitness),
                fatigue: round_metric(fatigue),
                form: round_metric(fitness - fatigue),
            });
        }
    }

    points
}

fn cached_row_query_start_date(requested_start_date: Option<NaiveDate>) -> Option<NaiveDate> {
    requested_start_date
}

fn parse_date(value: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| AppError::validation_field("date", "Dates must use YYYY-MM-DD format"))
}

fn round_metric(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{activities, analytics_user_states, fitness_freshness_daily};
    use chrono::DateTime;
    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, EntityTrait, Schema, Set,
    };

    async fn test_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory db");
        let schema = Schema::new(db.get_database_backend());

        db.execute(&schema.create_table_from_entity(activities::Entity))
            .await
            .expect("create activities table");
        db.execute(&schema.create_table_from_entity(analytics_user_states::Entity))
            .await
            .expect("create analytics user states table");
        db.execute(&schema.create_table_from_entity(fitness_freshness_daily::Entity))
            .await
            .expect("create fitness freshness table");

        db
    }

    #[test]
    fn parse_date_requires_iso_format() {
        let error = parse_date("05/01/2026").unwrap_err();

        assert_eq!(error.status, axum::http::StatusCode::BAD_REQUEST);
        assert_eq!(error.message, "Dates must use YYYY-MM-DD format");
    }

    #[test]
    fn cache_is_unusable_when_history_has_gaps() {
        let now = Utc::now();
        let rows = vec![
            fitness_freshness_daily::Model {
                id: 1,
                user_id: 1,
                day: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
                activity_count: 1,
                training_load: 10.0,
                fitness: 1.0,
                fatigue: 2.0,
                form: -1.0,
                created_at: now,
                updated_at: now,
            },
            fitness_freshness_daily::Model {
                id: 2,
                user_id: 1,
                day: NaiveDate::from_ymd_opt(2026, 5, 3).unwrap(),
                activity_count: 0,
                training_load: 0.0,
                fitness: 0.8,
                fatigue: 1.4,
                form: -0.6,
                created_at: now,
                updated_at: now,
            },
        ];

        assert!(!cache_is_usable(
            &rows,
            Some(&analytics_user_states::Model {
                user_id: 1,
                last_activity_change_at: now,
                fitness_dirty_from_day: None,
                last_fitness_rebuild_at: None,
                created_at: now,
                updated_at: now,
            }),
            Some(NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()),
            NaiveDate::from_ymd_opt(2026, 5, 3).unwrap(),
        ));
    }

    #[test]
    fn cache_is_unusable_when_dirty_window_reaches_request_end() {
        let now = Utc::now();
        let rows = vec![fitness_freshness_daily::Model {
            id: 1,
            user_id: 1,
            day: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            activity_count: 1,
            training_load: 10.0,
            fitness: 1.0,
            fatigue: 2.0,
            form: -1.0,
            created_at: now,
            updated_at: now,
        }];

        assert!(!cache_is_usable(
            &rows,
            Some(&analytics_user_states::Model {
                user_id: 1,
                last_activity_change_at: now,
                fitness_dirty_from_day: Some(NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()),
                last_fitness_rebuild_at: None,
                created_at: now,
                updated_at: now,
            }),
            Some(NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()),
            NaiveDate::from_ymd_opt(2026, 5, 2).unwrap(),
        ));
    }

    #[test]
    fn cache_is_usable_when_dirty_window_is_after_requested_end() {
        let now = Utc::now();
        let rows = vec![fitness_freshness_daily::Model {
            id: 1,
            user_id: 1,
            day: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            activity_count: 1,
            training_load: 10.0,
            fitness: 1.0,
            fatigue: 2.0,
            form: -1.0,
            created_at: now,
            updated_at: now,
        }];

        assert!(cache_is_usable(
            &rows,
            Some(&analytics_user_states::Model {
                user_id: 1,
                last_activity_change_at: now,
                fitness_dirty_from_day: Some(NaiveDate::from_ymd_opt(2026, 5, 5).unwrap()),
                last_fitness_rebuild_at: Some(now),
                created_at: now,
                updated_at: now,
            }),
            Some(NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()),
            NaiveDate::from_ymd_opt(2026, 5, 2).unwrap(),
        ));
    }

    #[test]
    fn cached_row_query_starts_at_requested_start_date() {
        assert_eq!(
            cached_row_query_start_date(Some(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap())),
            Some(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap())
        );
        assert_eq!(cached_row_query_start_date(None), None);
    }

    #[tokio::test]
    async fn load_cached_rows_for_request_rebuilds_dirty_cache_before_returning() {
        let db = test_db().await;
        let now = Utc::now();
        let day = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();

        activities::ActiveModel {
            user_id: Set(1),
            activity_import_id: Set(None),
            title: Set("Lunch Ride".to_string()),
            sport: Set("ride".to_string()),
            source: Set("manual_upload".to_string()),
            source_correlation_id: Set(None),
            original_filename: Set(None),
            format: Set(Some("fit".to_string())),
            started_at: Set(DateTime::parse_from_rfc3339("2026-05-01T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc)),
            ended_at: Set(None),
            distance_meters: Set(Some(25_000.0)),
            moving_time_seconds: Set(Some(3600)),
            total_time_seconds: Set(Some(3600)),
            elevation_gain_meters: Set(Some(400.0)),
            elevation_loss_meters: Set(Some(400.0)),
            average_speed_mps: Set(Some(7.0)),
            max_speed_mps: Set(Some(12.0)),
            average_heart_rate_bpm: Set(Some(150)),
            max_heart_rate_bpm: Set(Some(200)),
            average_cadence_rpm: Set(Some(85)),
            max_cadence_rpm: Set(Some(100)),
            calories: Set(Some(700)),
            estimated_ftp_watts: Set(None),
            heart_rate_zones_json: Set(None),
            derived_data_json: Set(None),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert activity");

        fitness_freshness_daily::ActiveModel {
            user_id: Set(1),
            day: Set(day),
            activity_count: Set(2),
            training_load: Set(112.5),
            fitness: Set(2.6785714285714284),
            fatigue: Set(16.071428571428573),
            form: Set(-13.392857142857144),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert stale cached row");

        analytics_user_states::ActiveModel {
            user_id: Set(1),
            last_activity_change_at: Set(now),
            fitness_dirty_from_day: Set(Some(day)),
            last_fitness_rebuild_at: Set(None),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert dirty analytics state");

        let rows = load_cached_rows_for_request(&db, 1, Some(day), day)
            .await
            .expect("load rows");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].activity_count, 1);
        assert!((rows[0].training_load - 56.25).abs() < f64::EPSILON);

        let freshness_state = analytics_user_states::Entity::find_by_id(1)
            .one(&db)
            .await
            .expect("load analytics state")
            .expect("analytics state exists");
        assert_eq!(freshness_state.fitness_dirty_from_day, None);
        assert!(freshness_state.last_fitness_rebuild_at.is_some());
    }
}
