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
    auth::UserContext,
    heatmaps::{
        raster::Tile,
        types::{HeatmapMetadata, HeatmapQuery, HeatmapZones},
    },
};
use std::sync::Arc;

#[utoipa::path(get, path="/maps/heatmap", params(HeatmapQuery),
    responses((status=200,description="Private heatmap filters, bounds and preparation progress",body=HeatmapMetadata),
        (status=400,description="Invalid filters",body=ApiErrorResponse),(status=401,description="Not authenticated"),(status=404,description="Heatmaps disabled")),
    tag="maps",security(("bearer_auth"=[])))]
pub async fn metadata(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Query(query): Query<HeatmapQuery>,
) -> Result<impl IntoResponse, AppError> {
    let metadata = state.heatmaps.metadata(&state.db, user.id, query).await?;
    Ok((
        [
            (header::CACHE_CONTROL, "private, no-store"),
            (header::VARY, "Cookie, Authorization"),
        ],
        axum::Json(metadata),
    ))
}

#[utoipa::path(get, path="/maps/heatmap/zones", params(HeatmapQuery),
    responses((status=200,description="Private route centers for zoomed-out heatmap zones",body=HeatmapZones),
        (status=400,description="Invalid filters or missing revision",body=ApiErrorResponse),(status=401,description="Not authenticated"),
        (status=404,description="Heatmaps disabled"),(status=409,description="Refresh metadata revision",body=ApiErrorResponse)),
    tag="maps",security(("bearer_auth"=[])))]
pub async fn zones(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Query(query): Query<HeatmapQuery>,
) -> Result<impl IntoResponse, AppError> {
    let zones = state.heatmaps.zones(&state.db, user.id, query).await?;
    Ok((
        [
            (header::CACHE_CONTROL, "private, no-store"),
            (header::VARY, "Cookie, Authorization"),
        ],
        axum::Json(zones),
    ))
}

#[utoipa::path(get, path="/maps/heatmap/tiles/{z}/{x}/{y}.png",
    params(HeatmapQuery,("z"=u8,Path,description="Zoom 0 through 18"),("x"=u32,Path),("y"=u32,Path)),
    responses((status=200,description="Transparent private 512-pixel heatmap PNG",content_type="image/png",body=Vec<u8>),
        (status=304,description="Authorized tile unchanged"),(status=400,description="Invalid filters or tile",body=ApiErrorResponse),
        (status=401,description="Not authenticated"),(status=404,description="Heatmaps disabled"),(status=409,description="Refresh metadata revision",body=ApiErrorResponse),
        (status=503,description="Render deadline exceeded; retry shortly",body=ApiErrorResponse)),tag="maps",security(("bearer_auth"=[])))]
pub async fn tile(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Path((z, x, y)): Path<(u8, u32, String)>,
    Query(query): Query<HeatmapQuery>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let y = y
        .strip_suffix(".png")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| AppError::bad_request("Invalid tile filename"))?;
    let image = state
        .heatmaps
        .tile(&state.db, user.id, query, Tile { z, x, y })
        .await?;
    let unchanged = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|v| v.trim() == image.etag));
    let mut response = if unchanged {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        Body::from(image.png.to_vec()).into_response()
    };
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, "image/png".parse().unwrap());
    headers.insert(header::CACHE_CONTROL, "private, no-cache".parse().unwrap());
    headers.insert(header::VARY, "Cookie, Authorization".parse().unwrap());
    headers.insert(header::ETAG, image.etag.parse().unwrap());
    Ok(response)
}
