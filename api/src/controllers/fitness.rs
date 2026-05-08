use crate::analytics::{
    FATIGUE_WINDOW_DAYS, FITNESS_WINDOW_DAYS, build_fitness_freshness_rows,
    default_fitness_rebuild_start_date,
};
use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::{activities, analytics_user_states, fitness_freshness_daily};
use crate::storage::AppStorage;
use axum::Json;
use axum::extract::{Query, State};
use chrono::{DateTime, Duration, NaiveDate, Utc};
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

    let cached_rows = fitness_freshness_daily::Entity::find()
        .filter(fitness_freshness_daily::Column::UserId.eq(user.id))
        .filter(fitness_freshness_daily::Column::Day.lte(end_date))
        .order_by_asc(fitness_freshness_daily::Column::Day)
        .all(&state.db)
        .await?;
    let freshness_state = analytics_user_states::Entity::find_by_id(user.id)
        .one(&state.db)
        .await?;

    let (start_date, points) =
        if cache_is_usable(&cached_rows, freshness_state.as_ref(), requested_start_date) {
            let start_date = requested_start_date.unwrap_or(cached_rows[0].day);

            (
                start_date,
                build_points_from_cached_rows(&cached_rows, start_date, end_date),
            )
        } else {
            state.tasks.rebuild_fitness_freshness(user.id).await;
            load_points_from_activities(&state.db, user.id, requested_start_date, end_date).await?
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
) -> bool {
    if rows.is_empty() {
        return false;
    }

    let Some(freshness_state) = freshness_state else {
        return false;
    };

    if requested_start_date.is_some_and(|start_date| start_date < rows[0].day) {
        return false;
    }

    if rows
        .windows(2)
        .any(|window| window[1].day != window[0].day + Duration::days(1))
    {
        return false;
    }

    rows.iter()
        .map(|row| row.updated_at)
        .min()
        .is_some_and(|updated_at| updated_at >= freshness_state.last_activity_change_at)
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

async fn load_points_from_activities(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    requested_start_date: Option<NaiveDate>,
    end_date: NaiveDate,
) -> Result<(NaiveDate, Vec<FitnessFreshnessPoint>), AppError> {
    let end_bound = end_of_day_utc(end_date)?;
    let activity_models = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::StartedAt.lte(end_bound))
        .order_by_asc(activities::Column::StartedAt)
        .all(db)
        .await?;
    let default_start_date = default_fitness_rebuild_start_date(&activity_models, end_date);
    let start_date = requested_start_date.unwrap_or(default_start_date);
    let compute_start_date = default_start_date.min(start_date);
    let rows = build_fitness_freshness_rows(&activity_models, compute_start_date, end_date);

    Ok((
        start_date,
        rows.into_iter()
            .filter(|row| row.day >= start_date)
            .map(|row| FitnessFreshnessPoint {
                date: row.day.format("%Y-%m-%d").to_string(),
                training_load: round_metric(row.training_load),
                fitness: round_metric(row.fitness),
                fatigue: round_metric(row.fatigue),
                form: round_metric(row.form),
            })
            .collect(),
    ))
}

fn parse_date(value: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| AppError::validation_field("date", "Dates must use YYYY-MM-DD format"))
}

fn end_of_day_utc(date: NaiveDate) -> Result<DateTime<Utc>, AppError> {
    let naive = date
        .and_hms_opt(23, 59, 59)
        .ok_or_else(|| AppError::validation_field("end_date", "Invalid end date"))?;

    Ok(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
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
                created_at: now,
                updated_at: now,
            }),
            Some(NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()),
        ));
    }

    #[test]
    fn cache_is_unusable_when_rows_are_older_than_latest_activity_change() {
        let row_time = Utc::now();
        let freshness_time = row_time + Duration::days(1);
        let rows = vec![fitness_freshness_daily::Model {
            id: 1,
            user_id: 1,
            day: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            activity_count: 1,
            training_load: 10.0,
            fitness: 1.0,
            fatigue: 2.0,
            form: -1.0,
            created_at: row_time,
            updated_at: row_time,
        }];

        assert!(!cache_is_usable(
            &rows,
            Some(&analytics_user_states::Model {
                user_id: 1,
                last_activity_change_at: freshness_time,
                created_at: freshness_time,
                updated_at: freshness_time,
            }),
            Some(NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()),
        ));
    }
}
