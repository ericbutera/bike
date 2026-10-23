use crate::app_error::{ApiErrorResponse, AppError};
use crate::storage::AppStorage;
use axum::extract::{Path, State};
use axum::Json;
use bike_core::auth::entities::users;
use bike_core::auth::UserContext;
use bike_core::entities::{segment_overall_achievements, segments};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct OverallAchievementResponse {
    /// Current global all-time position, with one athlete per position.
    #[schema(minimum = 1, maximum = 10, example = 1)]
    pub position: i32,
    #[schema(example = 7)]
    pub user_id: i32,
    pub rider_name: Option<String>,
    #[schema(example = 90)]
    pub effort_id: i32,
    #[schema(example = 355)]
    pub duration_seconds: i32,
}

impl OverallAchievementResponse {
    fn from_saved(saved: segment_overall_achievements::Model, names: &HashMap<i32, String>) -> Self {
        Self {
            position: saved.position,
            user_id: saved.user_id,
            rider_name: names.get(&saved.user_id).cloned(),
            effort_id: saved.effort_id,
            duration_seconds: saved.duration_seconds,
        }
    }
}

#[utoipa::path(
    get,
    path = "/segments/{id}/overall-achievements",
    params(("id" = i32, Path, description = "Segment ID", example = 7)),
    responses(
        (status = 200, description = "Saved current global all-time top ten, one position per athlete", body = [OverallAchievementResponse]),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Segment not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(("bearer_auth" = []))
)]
pub async fn get_overall_achievements(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<Vec<OverallAchievementResponse>>, AppError> {
    segments::Model::find_owned(&state.db, id, user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Segment not found"))?;
    let saved = segment_overall_achievements::Model::for_segment(&state.db, id).await?;
    if saved.is_empty() {
        return Ok(Json(Vec::new()));
    }
    let user_ids = saved.iter().map(|row| row.user_id).collect::<Vec<_>>();
    let names = users::Model::rider_names(&state.db, &user_ids)
        .await?
        .into_iter()
        .map(|rider| (rider.id, rider.name))
        .collect();
    Ok(Json(
        saved
            .into_iter()
            .map(|row| OverallAchievementResponse::from_saved(row, &names))
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controllers::test_support::local_user_json;
    use axum::{body::Body, http::Request};
    use bike_core::test_support as fixtures;
    use serde_json::json;
    use tower::ServiceExt;

    const PATH: &str = "/api/segments/1/overall-achievements";

    #[tokio::test]
    async fn returns_saved_global_rows_including_other_athletes_without_recalculating() {
        let rows = vec![
            fixtures::overall_achievement(),
            segment_overall_achievements::Model {
                position: 7,
                user_id: 8,
                effort_id: 18,
                duration_seconds: 400,
                ..fixtures::overall_achievement()
            },
        ];
        let db = fixtures::postgres_mock()
            .append_query_results([vec![fixtures::customer()]])
            .append_query_results([vec![fixtures::segment()]])
            .append_query_results([rows])
            .append_query_results([vec![fixtures::customer()]])
            .into_connection();
        let result = local_user_json(db.clone(), PATH, 200).await;
        assert_eq!(result[0]["position"], 1);
        assert_eq!(result[0]["user_id"], fixtures::segment_effort().user_id);
        assert_eq!(result[0]["rider_name"], fixtures::customer().name);
        assert_eq!(
            result[1],
            json!({"position": 7, "user_id": 8, "effort_id": 18, "duration_seconds": 400, "rider_name": null})
        );
        let log = db.into_transaction_log();
        assert_eq!(
            log.len(),
            4,
            "only authentication, segment access, saved positions and profile names"
        );
        assert_eq!(log[2].statements()[0].values.as_ref().unwrap().0, vec![1.into()]);
    }

    #[tokio::test]
    async fn missing_saved_output_returns_an_empty_list() {
        let db = fixtures::postgres_mock()
            .append_query_results([vec![fixtures::customer()]])
            .append_query_results([vec![fixtures::segment()]])
            .append_query_results([Vec::<segment_overall_achievements::Model>::new()])
            .into_connection();
        assert_eq!(local_user_json(db, PATH, 200).await, json!([]));
    }

    #[tokio::test]
    async fn inaccessible_segments_do_not_read_standings() {
        let db = fixtures::postgres_mock()
            .append_query_results([vec![fixtures::customer()]])
            .append_query_results([Vec::<segments::Model>::new()])
            .into_connection();
        local_user_json(db.clone(), PATH, 404).await;
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 2);
        assert_eq!(
            log[1].statements()[0].values.as_ref().unwrap().0,
            vec![1.into(), fixtures::customer().id.into(), 1_u64.into()]
        );
    }

    #[tokio::test]
    async fn saved_output_failures_are_reported() {
        let db = fixtures::postgres_mock()
            .append_query_results([vec![fixtures::customer()]])
            .append_query_results([vec![fixtures::segment()]])
            .append_query_errors([sea_orm::DbErr::Custom("fixture output unavailable".into())])
            .into_connection();
        local_user_json(db, PATH, 500).await;
    }

    #[tokio::test]
    async fn authentication_is_required() {
        let state = AppStorage::for_test(fixtures::postgres_mock().into_connection());
        let response = crate::controllers::routes()
            .with_state(Arc::new(state))
            .oneshot(Request::builder().uri(PATH).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
    }
}
