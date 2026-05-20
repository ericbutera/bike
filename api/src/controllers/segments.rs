use crate::activity_details::{
    derive_activity_detail_data, deserialize_derived_activity_data, ActivityRoutePoint,
};
use crate::activity_summary::summarize_activity_upload;
use crate::analytics::mark_segment_activity_changes;
use crate::app_error::{ApiErrorResponse, AppError};
use crate::dedupe::{segment_dedupe_key, segment_dedupe_key_from_model};
use crate::entities::{
    activities, segment_efforts, segment_summaries, segment_user_summaries, segments,
};
use crate::segment_support::{
    deserialize_segment_route_points, replace_segment_efforts_for_segment,
    serialize_segment_route_points, slice_effort_route_points,
};
use crate::storage::AppStorage;
use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use kaleido::auth::entities::users;
use kaleido::auth::UserContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct SegmentResponse {
    pub id: i32,
    pub title: String,
    pub source: String,
    pub original_filename: Option<String>,
    pub format: Option<String>,
    pub distance_meters: Option<f64>,
    pub effort_count: i32,
    pub best_duration_seconds: Option<i32>,
    pub current_user_pr_duration_seconds: Option<i32>,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_points: Vec<ActivityRoutePoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub efforts: Vec<SegmentEffortResponse>,
}

fn current_user_pr_duration_from_responses(
    efforts: &[SegmentEffortResponse],
    user_id: i32,
) -> Option<i32> {
    efforts
        .iter()
        .filter(|effort| effort.rider_user_id == user_id)
        .map(|effort| effort.duration_seconds)
        .min()
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SegmentEffortResponse {
    pub id: i32,
    pub rider_user_id: i32,
    pub activity_id: i32,
    pub activity_title: String,
    pub rider_name: String,
    pub activity_started_at: DateTime<Utc>,
    pub effort_index: i32,
    pub duration_seconds: i32,
    pub start_elapsed_seconds: i32,
    pub end_elapsed_seconds: i32,
    pub distance_meters: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_points: Vec<ActivityRoutePoint>,
}

#[utoipa::path(
    get,
    path = "/api/segments",
    responses(
        (status = 200, description = "Recent segments for the authenticated user", body = [SegmentResponse]),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_segments(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<Vec<SegmentResponse>>, AppError> {
    let mut segment_models = segments::Entity::find()
        .filter(segments::Column::UserId.eq(user.id))
        .all(&state.db)
        .await?;

    let segment_ids = segment_models
        .iter()
        .map(|segment| segment.id)
        .collect::<Vec<_>>();
    let summary_by_segment_id = load_segment_summaries(&state.db, &segment_ids).await?;
    sort_segments_by_latest_activity_started_at(&mut segment_models, &summary_by_segment_id);
    let user_summary_by_segment_id =
        load_segment_user_summaries(&state.db, user.id, &segment_ids).await?;
    let stale_segment_ids = segment_models
        .iter()
        .filter_map(|segment| {
            let summary = summary_by_segment_id.get(&segment.id);
            let user_summary = user_summary_by_segment_id.get(&segment.id);

            match (summary, user_summary) {
                (Some(summary), Some(user_summary))
                    if summary.updated_at >= segment.last_activity_change_at
                        && user_summary.updated_at >= segment.last_activity_change_at =>
                {
                    None
                }
                (Some(summary), None) if summary.updated_at >= segment.last_activity_change_at => {
                    None
                }
                _ => Some(segment.id),
            }
        })
        .collect::<Vec<_>>();

    if !stale_segment_ids.is_empty() {
        state
            .tasks
            .rebuild_segment_analytics(stale_segment_ids)
            .await;
    }

    Ok(Json(
        segment_models
            .into_iter()
            .map(|segment| {
                let summary = summary_by_segment_id.get(&segment.id);
                let user_summary = user_summary_by_segment_id.get(&segment.id);

                SegmentResponse {
                    id: segment.id,
                    title: segment.title,
                    source: segment.source,
                    original_filename: segment.original_filename,
                    format: segment.format,
                    distance_meters: segment.distance_meters,
                    effort_count: summary.map(|value| value.effort_count).unwrap_or_default(),
                    best_duration_seconds: summary.and_then(|value| value.best_duration_seconds),
                    current_user_pr_duration_seconds: user_summary
                        .and_then(|value| value.personal_best_duration_seconds),
                    created_at: segment.created_at,
                    route_points: Vec::new(),
                    efforts: Vec::new(),
                }
            })
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/segments/{id}",
    params(
        ("id" = i32, Path, description = "Segment ID")
    ),
    responses(
        (status = 200, description = "Segment detail and effort comparison data", body = SegmentResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Segment not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_segment(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<SegmentResponse>, AppError> {
    let segment = segments::Entity::find()
        .filter(segments::Column::Id.eq(id))
        .filter(segments::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Segment not found"))?;
    let route_points = deserialize_segment_route_points(segment.route_data_json.as_ref());
    let efforts = load_effort_responses(&state.db, &[segment.id]).await?;

    Ok(Json(SegmentResponse {
        id: segment.id,
        title: segment.title,
        source: segment.source,
        original_filename: segment.original_filename,
        format: segment.format,
        distance_meters: segment.distance_meters,
        effort_count: efforts.len() as i32,
        best_duration_seconds: efforts.iter().map(|effort| effort.duration_seconds).min(),
        current_user_pr_duration_seconds: current_user_pr_duration_from_responses(
            &efforts, user.id,
        ),
        created_at: segment.created_at,
        route_points,
        efforts,
    }))
}

#[utoipa::path(
    post,
    path = "/api/segments",
    request_body(content_type = "multipart/form-data"),
    responses(
        (status = 201, description = "Segment imported and matched to recent activities", body = SegmentResponse),
        (status = 400, description = "Invalid upload", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn import_segment(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<SegmentResponse>), AppError> {
    let upload = read_uploaded_segment_file(multipart).await?;
    let segment_summary =
        summarize_activity_upload(&upload.original_filename, &upload.format, &upload.bytes)?;
    let segment_detail =
        derive_activity_detail_data(&upload.original_filename, &upload.format, &upload.bytes)?;

    if segment_detail.route_points.len() < 2 {
        return Err(AppError::validation_field(
            "file",
            "Segment imports require GPX or TCX route data with coordinates",
        ));
    }

    if let Some(existing_segment) = find_duplicate_segment(
        &state.db,
        user.id,
        segment_summary.distance_meters,
        &segment_detail.route_points,
    )
    .await?
    {
        let efforts = load_effort_responses(&state.db, &[existing_segment.id]).await?;

        return Ok((
            StatusCode::OK,
            Json(SegmentResponse {
                id: existing_segment.id,
                title: existing_segment.title,
                source: existing_segment.source,
                original_filename: existing_segment.original_filename,
                format: existing_segment.format,
                distance_meters: existing_segment.distance_meters,
                effort_count: efforts.len() as i32,
                best_duration_seconds: efforts.iter().map(|effort| effort.duration_seconds).min(),
                current_user_pr_duration_seconds: current_user_pr_duration_from_responses(
                    &efforts, user.id,
                ),
                created_at: existing_segment.created_at,
                route_points: deserialize_segment_route_points(
                    existing_segment.route_data_json.as_ref(),
                ),
                efforts,
            }),
        ));
    }

    let segment = segments::ActiveModel {
        user_id: Set(user.id),
        title: Set(segment_summary.title),
        source: Set("manual_segment_import".to_string()),
        original_filename: Set(Some(upload.original_filename)),
        format: Set(Some(upload.format)),
        distance_meters: Set(segment_summary.distance_meters),
        route_data_json: Set(Some(serialize_segment_route_points(
            &segment_detail.route_points,
        )?)),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;

    replace_segment_efforts_for_segment(&state.db, segment.id, &segment_detail.route_points)
        .await?;
    let changed_at = Utc::now();
    mark_segment_activity_changes(&state.db, &[segment.id], changed_at).await?;
    state
        .tasks
        .rebuild_segment_analytics(vec![segment.id])
        .await;
    let efforts = load_effort_responses(&state.db, &[segment.id]).await?;

    Ok((
        StatusCode::CREATED,
        Json(SegmentResponse {
            id: segment.id,
            title: segment.title,
            source: segment.source,
            original_filename: segment.original_filename,
            format: segment.format,
            distance_meters: segment.distance_meters,
            effort_count: efforts.len() as i32,
            best_duration_seconds: efforts.iter().map(|effort| effort.duration_seconds).min(),
            current_user_pr_duration_seconds: current_user_pr_duration_from_responses(
                &efforts, user.id,
            ),
            created_at: segment.created_at,
            route_points: deserialize_segment_route_points(segment.route_data_json.as_ref()),
            efforts,
        }),
    ))
}

struct UploadedSegmentFile {
    original_filename: String,
    format: String,
    bytes: Vec<u8>,
}

async fn read_uploaded_segment_file(
    mut multipart: Multipart,
) -> Result<UploadedSegmentFile, AppError> {
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::bad_request(format!("Malformed multipart payload: {error}")))?
    {
        if field.name() != Some("file") && field.file_name().is_none() {
            continue;
        }

        let original_filename = field
            .file_name()
            .map(|value| value.to_string())
            .ok_or_else(|| {
                AppError::validation_field("file", "Uploaded file is missing a filename")
            })?;
        let format = validate_segment_format(&original_filename)?;
        let mut bytes = Vec::new();

        while let Some(chunk) = field.chunk().await.map_err(|error| {
            AppError::bad_request(format!("Failed to read segment upload: {error}"))
        })? {
            bytes.extend_from_slice(&chunk);
        }

        if bytes.is_empty() {
            return Err(AppError::validation_field("file", "Uploaded file is empty"));
        }

        return Ok(UploadedSegmentFile {
            original_filename,
            format,
            bytes,
        });
    }

    Err(AppError::validation_field(
        "file",
        "A GPX or TCX file is required to import a segment",
    ))
}

fn validate_segment_format(filename: &str) -> Result<String, AppError> {
    let extension = std::path::Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| {
            AppError::validation_field(
                "file",
                "Segments currently require a GPX or TCX export with route coordinates",
            )
        })?;

    match extension.as_str() {
        "gpx" | "tcx" => Ok(extension),
        _ => Err(AppError::validation_field(
            "file",
            "Segments currently require a GPX or TCX export with route coordinates",
        )),
    }
}

async fn find_duplicate_segment(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    distance_meters: Option<f64>,
    route_points: &[ActivityRoutePoint],
) -> Result<Option<segments::Model>, AppError> {
    let Some(target_key) = segment_dedupe_key(distance_meters, route_points) else {
        return Ok(None);
    };

    let candidates = segments::Entity::find()
        .filter(segments::Column::UserId.eq(user_id))
        .all(db)
        .await?;

    for segment in candidates {
        if segment_dedupe_key_from_model(&segment).as_deref() == Some(target_key.as_str()) {
            return Ok(Some(segment));
        }
    }

    Ok(None)
}

fn sort_segments_by_latest_activity_started_at(
    segment_models: &mut [segments::Model],
    summary_by_segment_id: &HashMap<i32, segment_summaries::Model>,
) {
    segment_models.sort_by(|left, right| {
        let left_latest = summary_by_segment_id
            .get(&left.id)
            .and_then(|summary| summary.latest_activity_started_at);
        let right_latest = summary_by_segment_id
            .get(&right.id)
            .and_then(|summary| summary.latest_activity_started_at);

        right_latest
            .cmp(&left_latest)
            .then_with(|| right.created_at.cmp(&left.created_at))
            .then_with(|| right.id.cmp(&left.id))
    });
}

async fn load_segment_summaries(
    db: &sea_orm::DatabaseConnection,
    segment_ids: &[i32],
) -> Result<HashMap<i32, segment_summaries::Model>, AppError> {
    if segment_ids.is_empty() {
        return Ok(HashMap::new());
    }

    Ok(segment_summaries::Entity::find()
        .filter(segment_summaries::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .all(db)
        .await?
        .into_iter()
        .map(|summary| (summary.segment_id, summary))
        .collect())
}

async fn load_segment_user_summaries(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    segment_ids: &[i32],
) -> Result<HashMap<i32, segment_user_summaries::Model>, AppError> {
    if segment_ids.is_empty() {
        return Ok(HashMap::new());
    }

    Ok(segment_user_summaries::Entity::find()
        .filter(segment_user_summaries::Column::UserId.eq(user_id))
        .filter(segment_user_summaries::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .all(db)
        .await?
        .into_iter()
        .map(|summary| (summary.segment_id, summary))
        .collect())
}

async fn load_effort_responses(
    db: &sea_orm::DatabaseConnection,
    segment_ids: &[i32],
) -> Result<Vec<SegmentEffortResponse>, AppError> {
    if segment_ids.is_empty() {
        return Ok(Vec::new());
    }

    let efforts = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .order_by_asc(segment_efforts::Column::DurationSeconds)
        .order_by_asc(segment_efforts::Column::Id)
        .all(db)
        .await?;
    let activity_ids = efforts
        .iter()
        .map(|effort| effort.activity_id)
        .collect::<Vec<_>>();
    let rider_user_ids = efforts
        .iter()
        .map(|effort| effort.user_id)
        .collect::<Vec<_>>();
    let activity_models = activities::Entity::find()
        .filter(activities::Column::Id.is_in(activity_ids.iter().copied()))
        .all(db)
        .await?;
    let rider_models = users::Entity::find()
        .filter(users::Column::Id.is_in(rider_user_ids.iter().copied()))
        .all(db)
        .await?;
    let activities_by_id = activity_models
        .into_iter()
        .map(|activity| (activity.id, activity))
        .collect::<HashMap<_, _>>();
    let riders_by_id = rider_models
        .into_iter()
        .map(|rider| (rider.id, rider))
        .collect::<HashMap<_, _>>();

    Ok(efforts
        .into_iter()
        .filter_map(|effort| {
            let activity = activities_by_id.get(&effort.activity_id)?;
            let rider = riders_by_id.get(&effort.user_id)?;
            let derived_data =
                deserialize_derived_activity_data(activity.derived_data_json.as_ref());

            Some(SegmentEffortResponse {
                id: effort.id,
                rider_user_id: effort.user_id,
                activity_id: effort.activity_id,
                activity_title: activity.title.clone(),
                rider_name: rider.name.clone(),
                activity_started_at: activity.started_at,
                effort_index: effort.effort_index,
                duration_seconds: effort.duration_seconds,
                start_elapsed_seconds: effort.start_elapsed_seconds,
                end_elapsed_seconds: effort.end_elapsed_seconds,
                distance_meters: effort.distance_meters,
                route_points: slice_effort_route_points(
                    &derived_data.route_points,
                    effort.start_route_point_index,
                    effort.end_route_point_index,
                ),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn current_user_pr_duration_from_models(
        efforts: &[segment_efforts::Model],
        user_id: i32,
    ) -> Option<i32> {
        efforts
            .iter()
            .filter(|effort| effort.user_id == user_id)
            .map(|effort| effort.duration_seconds)
            .min()
    }

    fn build_segment_model(id: i32, created_at: DateTime<Utc>) -> segments::Model {
        segments::Model {
            id,
            user_id: 1,
            title: format!("Segment {id}"),
            source: "manual_segment_import".to_string(),
            original_filename: None,
            format: Some("gpx".to_string()),
            distance_meters: Some(1800.0),
            route_data_json: None,
            last_activity_change_at: created_at,
            created_at,
            updated_at: created_at,
        }
    }

    #[test]
    fn validate_segment_format_accepts_route_files() {
        assert_eq!(validate_segment_format("climb.gpx").unwrap(), "gpx");
        assert_eq!(validate_segment_format("climb.tcx").unwrap(), "tcx");
    }

    #[test]
    fn validate_segment_format_rejects_fit_for_now() {
        let error = validate_segment_format("climb.fit").unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.message,
            "Segments currently require a GPX or TCX export with route coordinates"
        );
    }

    #[test]
    fn loads_current_user_pr_duration_from_models() {
        let now = Utc::now();
        let efforts = vec![
            segment_efforts::Model {
                id: 1,
                segment_id: 10,
                user_id: 7,
                activity_id: 100,
                effort_index: 1,
                duration_seconds: 320,
                start_elapsed_seconds: 0,
                end_elapsed_seconds: 320,
                start_route_point_index: 0,
                end_route_point_index: 1,
                distance_meters: Some(1800.0),
                overall_rank: None,
                user_rank: None,
                created_at: now,
                updated_at: now,
            },
            segment_efforts::Model {
                id: 2,
                segment_id: 10,
                user_id: 7,
                activity_id: 101,
                effort_index: 2,
                duration_seconds: 305,
                start_elapsed_seconds: 0,
                end_elapsed_seconds: 305,
                start_route_point_index: 0,
                end_route_point_index: 1,
                distance_meters: Some(1800.0),
                overall_rank: None,
                user_rank: None,
                created_at: now,
                updated_at: now,
            },
            segment_efforts::Model {
                id: 3,
                segment_id: 10,
                user_id: 9,
                activity_id: 102,
                effort_index: 1,
                duration_seconds: 300,
                start_elapsed_seconds: 0,
                end_elapsed_seconds: 300,
                start_route_point_index: 0,
                end_route_point_index: 1,
                distance_meters: Some(1800.0),
                overall_rank: None,
                user_rank: None,
                created_at: now,
                updated_at: now,
            },
        ];

        assert_eq!(current_user_pr_duration_from_models(&efforts, 7), Some(305));
        assert_eq!(current_user_pr_duration_from_models(&efforts, 11), None);
    }

    #[test]
    fn sorts_segments_by_latest_activity_started_at_then_created_at() {
        let base_time = Utc::now();
        let mut segment_models = vec![
            build_segment_model(1, base_time + Duration::minutes(1)),
            build_segment_model(2, base_time + Duration::minutes(2)),
            build_segment_model(3, base_time + Duration::minutes(3)),
            build_segment_model(4, base_time + Duration::minutes(4)),
        ];
        let summary_by_segment_id = HashMap::from([
            (
                1,
                segment_summaries::Model {
                    segment_id: 1,
                    effort_count: 3,
                    leader_user_id: Some(9),
                    leader_effort_id: Some(11),
                    best_duration_seconds: Some(300),
                    latest_activity_started_at: Some(base_time + Duration::days(1)),
                    latest_activity_id: Some(101),
                    latest_effort_id: Some(201),
                    created_at: base_time,
                    updated_at: base_time,
                },
            ),
            (
                2,
                segment_summaries::Model {
                    segment_id: 2,
                    effort_count: 5,
                    leader_user_id: Some(9),
                    leader_effort_id: Some(12),
                    best_duration_seconds: Some(290),
                    latest_activity_started_at: Some(base_time + Duration::days(2)),
                    latest_activity_id: Some(102),
                    latest_effort_id: Some(202),
                    created_at: base_time,
                    updated_at: base_time,
                },
            ),
            (
                3,
                segment_summaries::Model {
                    segment_id: 3,
                    effort_count: 4,
                    leader_user_id: Some(8),
                    leader_effort_id: Some(13),
                    best_duration_seconds: Some(295),
                    latest_activity_started_at: Some(base_time + Duration::days(2)),
                    latest_activity_id: Some(103),
                    latest_effort_id: Some(203),
                    created_at: base_time,
                    updated_at: base_time,
                },
            ),
        ]);

        sort_segments_by_latest_activity_started_at(&mut segment_models, &summary_by_segment_id);

        assert_eq!(
            segment_models
                .into_iter()
                .map(|segment| segment.id)
                .collect::<Vec<_>>(),
            vec![3, 2, 1, 4]
        );
    }
}
