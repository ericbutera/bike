use crate::app_error::{ApiErrorResponse, AppError};
use crate::storage::AppStorage;
use axum::extract::{Path, State};
use axum::Json;
use bike_core::auth::UserContext;
use bike_core::entities::{segment_personal_achievements, segments};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct PersonalAchievementResponse {
    /// All-time personal position: 1 is PR, 2 is second, 3 is third.
    #[schema(minimum = 1, maximum = 3, example = 1)]
    pub position: i32,
    #[schema(example = 90)]
    pub effort_id: i32,
    #[schema(example = 355)]
    pub duration_seconds: i32,
}

impl From<segment_personal_achievements::Model> for PersonalAchievementResponse {
    fn from(saved: segment_personal_achievements::Model) -> Self {
        Self {
            position: saved.position,
            effort_id: saved.effort_id,
            duration_seconds: saved.duration_seconds,
        }
    }
}

#[utoipa::path(
    get,
    path = "/segments/{id}/personal-achievements",
    params(("id" = i32, Path, description = "Segment ID", example = 7)),
    responses(
        (status = 200, description = "Saved all-time personal positions for the authenticated rider", body = [PersonalAchievementResponse]),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Segment not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(("bearer_auth" = []))
)]
pub async fn get_personal_achievements(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<Vec<PersonalAchievementResponse>>, AppError> {
    segments::Model::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::not_found("Segment not found"))?;
    let saved = segment_personal_achievements::Model::for_user_segment(&state.db, user.id, id).await?;
    Ok(Json(saved.into_iter().map(PersonalAchievementResponse::from).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controllers::test_support::local_user_json;
    use axum::body::Body;
    use axum::http::Request;
    use bike_core::test_support as fixtures;
    use sea_orm::DbErr;
    use serde_json::json;
    use tower::ServiceExt;

    const PATH: &str = "/api/segments/1/personal-achievements";

    #[tokio::test]
    async fn returns_saved_positions_scoped_to_the_authenticated_rider() {
        let rows = (1..=3)
            .map(|position| segment_personal_achievements::Model {
                position,
                effort_id: 10 + position,
                duration_seconds: 300 + position,
                ..fixtures::personal_achievement()
            })
            .collect::<Vec<_>>();
        let db = fixtures::postgres_mock()
            .append_query_results([vec![fixtures::customer()]])
            .append_query_results([vec![fixtures::segment()]])
            .append_query_results([rows])
            .into_connection();
        assert_eq!(
            local_user_json(db.clone(), PATH, 200).await,
            json!([
                {"position": 1, "effort_id": 11, "duration_seconds": 301},
                {"position": 2, "effort_id": 12, "duration_seconds": 302},
                {"position": 3, "effort_id": 13, "duration_seconds": 303}
            ])
        );
        let log = db.into_transaction_log();
        assert_eq!(
            log.len(),
            3,
            "only authentication, segment access and saved output reads"
        );
        let query = &log[2].statements()[0];
        assert_eq!(
            query.values.as_ref().unwrap().0,
            vec![1.into(), fixtures::customer().id.into()]
        );
        assert!(query
            .sql
            .contains("ORDER BY \"segment_personal_achievements\".\"position\" ASC"));
    }

    #[tokio::test]
    async fn returns_partial_and_empty_saved_results() {
        for rows in [vec![fixtures::personal_achievement()], Vec::new()] {
            let count = rows.len();
            let db = fixtures::postgres_mock()
                .append_query_results([vec![fixtures::customer()]])
                .append_query_results([vec![fixtures::segment()]])
                .append_query_results([rows])
                .into_connection();
            let result = local_user_json(db, PATH, 200).await;
            assert_eq!(result.as_array().unwrap().len(), count);
        }
    }

    #[tokio::test]
    async fn missing_segment_does_not_read_personal_results() {
        let db = fixtures::postgres_mock()
            .append_query_results([vec![fixtures::customer()]])
            .append_query_results([Vec::<segments::Model>::new()])
            .into_connection();
        local_user_json(db.clone(), PATH, 404).await;
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 2);
        assert_eq!(
            log[1].statements()[0].values.as_ref().unwrap().0,
            vec![1.into(), 1_u64.into()]
        );
    }

    #[tokio::test]
    async fn failed_saved_read_returns_an_error() {
        let db = fixtures::postgres_mock()
            .append_query_results([vec![fixtures::customer()]])
            .append_query_results([vec![fixtures::segment()]])
            .append_query_errors([DbErr::Custom("fixture saved results unavailable".into())])
            .into_connection();
        local_user_json(db, PATH, 500).await;
    }

    #[tokio::test]
    async fn requires_authentication() {
        let state = AppStorage::for_test(fixtures::postgres_mock().into_connection());
        let response = crate::controllers::routes()
            .with_state(Arc::new(state))
            .oneshot(Request::builder().uri(PATH).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
    }
}
