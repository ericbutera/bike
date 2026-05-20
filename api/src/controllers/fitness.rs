use crate::analytics::{FATIGUE_WINDOW_DAYS, FITNESS_WINDOW_DAYS};
use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::{analytics_user_states, fitness_freshness_daily};
use crate::storage::AppStorage;
use axum::extract::{Query, State};
use axum::Json;
use chrono::{Duration, NaiveDate, Utc};
use kaleido::auth::UserContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
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

    let mut cached_rows_query = fitness_freshness_daily::Entity::find()
        .filter(fitness_freshness_daily::Column::UserId.eq(user.id))
        .filter(fitness_freshness_daily::Column::Day.lte(end_date))
        .order_by_asc(fitness_freshness_daily::Column::Day);

    if let Some(start_date) = cached_row_query_start_date(requested_start_date) {
        cached_rows_query = cached_rows_query
            .filter(fitness_freshness_daily::Column::Day.gte(start_date));
    }

    let cached_rows = cached_rows_query.all(&state.db).await?;
    let freshness_state = analytics_user_states::Entity::find_by_id(user.id)
        .one(&state.db)
        .await?;

    let start_date = requested_start_date
        .or_else(|| cached_rows.first().map(|row| row.day))
        .unwrap_or(end_date);
    let points = if cache_is_usable(
        &cached_rows,
        freshness_state.as_ref(),
        requested_start_date,
        end_date,
    ) {
        build_points_from_cached_rows(&cached_rows, start_date, end_date)
    } else {
        state.tasks.rebuild_fitness_freshness(user.id).await;

        if cached_rows.is_empty() {
            Vec::new()
        } else {
            build_points_from_cached_rows(&cached_rows, start_date, end_date)
        }
    };

    Ok(Json(FitnessFreshnessResponse {
        start_date: start_date.format("%Y-%m-%d").to_string(),
        end_date: end_date.format("%Y-%m-%d").to_string(),
        fitness_window_days: FITNESS_WINDOW_DAYS as i32,
        fatigue_window_days: FATIGUE_WINDOW_DAYS as i32,
        points,
    }))
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
}
