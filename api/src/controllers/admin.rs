use crate::analytics::mark_user_activity_changes;
use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::{activities, segments};
use crate::storage::AppStorage;
use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use chrono::Utc;
use kaleido::auth::AdminUserContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

const SEGMENT_BACKFILL_CHUNK_SIZE: usize = 250;

pub fn routes() -> Router<Arc<AppStorage>> {
    Router::new().route("/analytics/backfill", post(backfill_analytics))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AnalyticsBackfillResponse {
    pub user_count: i32,
    pub segment_count: i32,
    pub fitness_task_count: i32,
    pub segment_task_count: i32,
    pub total_tasks_enqueued: i32,
    pub segment_chunk_size: i32,
}

#[utoipa::path(
    post,
    path = "/admin/analytics/backfill",
    operation_id = "admin_backfill_analytics",
    responses(
        (status = 202, description = "Enqueued analytics backfill tasks", body = AnalyticsBackfillResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn backfill_analytics(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<(StatusCode, Json<AnalyticsBackfillResponse>), AppError> {
    let user_ids = load_user_ids_with_activities(&state).await?;
    let segment_ids = load_segment_ids(&state).await?;
    let changed_at = Utc::now();

    mark_user_activity_changes(&state.db, &user_ids, changed_at).await?;

    for user_id in &user_ids {
        state.tasks.rebuild_fitness_freshness(*user_id).await;
    }

    let segment_task_count = enqueue_segment_backfill_tasks(&state, &segment_ids).await;
    let fitness_task_count = user_ids.len() as i32;

    Ok((
        StatusCode::ACCEPTED,
        Json(AnalyticsBackfillResponse {
            user_count: user_ids.len() as i32,
            segment_count: segment_ids.len() as i32,
            fitness_task_count,
            segment_task_count,
            total_tasks_enqueued: fitness_task_count + segment_task_count,
            segment_chunk_size: SEGMENT_BACKFILL_CHUNK_SIZE as i32,
        }),
    ))
}

async fn load_user_ids_with_activities(state: &Arc<AppStorage>) -> Result<Vec<i32>, AppError> {
    let mut user_ids = activities::Entity::find()
        .select_only()
        .column(activities::Column::UserId)
        .filter(activities::Column::UserId.is_not_null())
        .distinct()
        .into_tuple::<i32>()
        .all(&state.db)
        .await?;
    user_ids.sort_unstable();
    user_ids.dedup();

    Ok(user_ids)
}

async fn load_segment_ids(state: &Arc<AppStorage>) -> Result<Vec<i32>, AppError> {
    let mut segment_ids = segments::Entity::find()
        .select_only()
        .column(segments::Column::Id)
        .into_tuple::<i32>()
        .all(&state.db)
        .await?;
    segment_ids.sort_unstable();
    segment_ids.dedup();

    Ok(segment_ids)
}

async fn enqueue_segment_backfill_tasks(state: &Arc<AppStorage>, segment_ids: &[i32]) -> i32 {
    let mut task_count = 0;

    for chunk in segment_ids.chunks(SEGMENT_BACKFILL_CHUNK_SIZE) {
        state.tasks.rebuild_segment_analytics(chunk.to_vec()).await;
        task_count += 1;
    }

    task_count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_segment_backfill_enqueues_no_tasks() {
        let state = Arc::new(AppStorage {
            db: sea_orm::Database::connect("sqlite::memory:")
                .await
                .expect("in-memory db"),
            tasks: crate::tasks::TaskQueue::new(
                sea_orm::Database::connect("sqlite::memory:")
                    .await
                    .expect("in-memory queue db"),
            ),
            feature_flags: kaleido::glass::feature_flags::FeatureFlagService::new(),
            auth_service: crate::tasks::create_auth_service(
                sea_orm::Database::connect("sqlite::memory:")
                    .await
                    .expect("in-memory auth db"),
                crate::tasks::TaskQueue::new(
                    sea_orm::Database::connect("sqlite::memory:")
                        .await
                        .expect("in-memory auth queue db"),
                ),
            ),
            uploads_dir: "/tmp".to_string(),
        });

        assert_eq!(enqueue_segment_backfill_tasks(&state, &[]).await, 0);
    }
}
