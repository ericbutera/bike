use crate::{
    app_error::{ApiErrorResponse, AppError},
    storage::AppStorage,
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use bike_core::{
    activity_maps::{MapOptions, Theme, Variant},
    auth::UserContext,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct ImageQuery {
    #[serde(rename = "activityId")]
    activity_id: i32,
    theme: Theme,
    dpr: u8,
}

#[utoipa::path(
    get, path = "/activity-map-images/{variant}/{style_version}",
    params(("variant" = Variant, Path), ("style_version" = String, Path),
        ("activityId" = i32, Query), ("theme" = Theme, Query), ("dpr" = u8, Query, minimum = 1, maximum = 2)),
    responses((status = 200, description = "Private activity map PNG", content_type = "image/png", body = Vec<u8>),
        (status = 304, description = "Owned activity map has not changed"),
        (status = 400, description = "Invalid map options", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Activity or route not found", body = ApiErrorResponse),
        (status = 502, description = "Snapshot worker unavailable", body = ApiErrorResponse)),
    tag = "activities", security(("bearer_auth" = []))
)]
pub async fn image(
    UserContext { user, .. }: UserContext<AppStorage>,
    Path((variant, style_version)): Path<(Variant, String)>,
    Query(query): Query<ImageQuery>,
    State(state): State<Arc<AppStorage>>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let service = state
        .map_images
        .as_ref()
        .ok_or_else(|| AppError::not_found("Activity maps are not configured"))?;
    let image = service
        .image(
            &state.db,
            user.id,
            query.activity_id,
            variant,
            &style_version,
            MapOptions {
                theme: query.theme,
                dpr: query.dpr,
            },
        )
        .await?;
    Ok(image_response(image, &headers))
}

fn image_response(image: bike_core::activity_maps::MapImage, headers: &HeaderMap) -> Response {
    let unchanged = headers
        .get(header::IF_NONE_MATCH)
        .is_some_and(|value| value.as_bytes() == image.etag.as_bytes());
    let status = if unchanged {
        StatusCode::NOT_MODIFIED
    } else {
        StatusCode::OK
    };
    let response_headers = [
        (header::CONTENT_TYPE, "image/png".to_owned()),
        (header::CACHE_CONTROL, "private, no-cache".to_owned()),
        (header::VARY, "Cookie, Authorization".to_owned()),
        (header::ETAG, image.etag),
        (
            axum::http::HeaderName::from_static("x-map-cache"),
            if image.cache_hit { "hit" } else { "miss" }.to_owned(),
        ),
    ];
    let body = if unchanged {
        Body::empty()
    } else {
        Body::from(image.png)
    };
    (status, response_headers, body).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;

    #[tokio::test]
    async fn unconfigured_map_service_returns_the_api_not_found_contract() {
        let pid = uuid::Uuid::new_v4();
        let user: bike_core::auth::entities::users::Model =
            serde_json::from_value(serde_json::json!({
                "id": 1, "pid": pid, "email": "map-test@example.invalid", "api_key": "fixture",
                "name": "Map test", "disabled": false,
                "created_at": "2026-10-09T00:00:00Z", "updated_at": "2026-10-09T00:00:00Z"
            }))
            .unwrap();
        let db = sea_orm::MockDatabase::new(sea_orm::DatabaseBackend::Postgres)
            .append_query_results([vec![user]])
            .into_connection();
        let mut state = AppStorage::for_test(db);
        state.local_admin_user_pid = Some(pid);
        let app = crate::controllers::routes().with_state(Arc::new(state));
        let request = axum::http::Request::builder()
            .uri("/api/activity-map-images/full/1?activityId=1&theme=light&dpr=1")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        assert!(std::str::from_utf8(&body)
            .unwrap()
            .contains("Activity maps are not configured"));
    }

    #[tokio::test]
    async fn private_png_and_conditional_response_share_cache_headers() {
        for (etag, expected) in [
            (None, StatusCode::OK),
            (Some("\"previous\""), StatusCode::OK),
            (Some("\"current\""), StatusCode::NOT_MODIFIED),
        ] {
            let image = bike_core::activity_maps::MapImage {
                png: b"png".to_vec(),
                etag: "\"current\"".into(),
                cache_hit: etag.is_some(),
            };
            let mut headers = HeaderMap::new();
            if let Some(etag) = etag {
                headers.insert(header::IF_NONE_MATCH, etag.parse().unwrap());
            }
            let response = image_response(image, &headers);
            assert_eq!(response.status(), expected);
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "private, no-cache"
            );
            assert_eq!(response.headers()[header::VARY], "Cookie, Authorization");
            assert_eq!(response.headers()[header::ETAG], "\"current\"");
            let body = axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap();
            assert_eq!(body.is_empty(), expected == StatusCode::NOT_MODIFIED);
        }
    }
}
