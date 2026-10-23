use super::{ActiveModel, Model, SegmentMode};
use crate::activity_details::{derive_activity_detail_data, deserialize_derived_activity_data, ActivityRoutePoint};
use crate::activity_summary::summarize_activity_upload;
use crate::entities::{activities, segments};
use crate::segment_support::{serialize_segment_route_points, slice_effort_route_points};
use crate::workflow_error::WorkflowError as AppError;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};

#[derive(Debug, Clone)]
pub struct ActivitySegmentSource {
    activity: activities::Model,
    title: String,
    route_points: Vec<ActivityRoutePoint>,
    distance_meters: Option<f64>,
    start_route_point_index: i32,
    end_route_point_index: i32,
}

#[derive(Debug)]
pub enum SegmentImport {
    Existing(Model),
    Created(Model),
}

impl ActivitySegmentSource {
    pub fn route_points(&self) -> &[ActivityRoutePoint] {
        &self.route_points
    }
    pub fn distance_meters(&self) -> Option<f64> {
        self.distance_meters
    }
    pub async fn load(
        db: &impl ConnectionTrait,
        user_id: i32,
        activity_id: i32,
        title: &str,
        start_route_point_index: i32,
        end_route_point_index: i32,
    ) -> Result<Self, AppError> {
        let activity = activities::Model::find_owned(db, activity_id, user_id)
            .await?
            .ok_or_else(|| AppError::not_found("Activity not found"))?;
        let title = normalize_segment_title(title)?;
        let activity_route_points = deserialize_derived_activity_data(activity.derived_data_json.as_ref()).route_points;
        let route_points =
            slice_builder_route_points(&activity_route_points, start_route_point_index, end_route_point_index)?;
        let distance_meters = route_points.last().and_then(|point| point.distance_meters);

        Ok(Self {
            activity,
            title,
            route_points,
            distance_meters,
            start_route_point_index,
            end_route_point_index,
        })
    }
}

pub fn normalize_segment_title(raw_title: &str) -> Result<String, AppError> {
    let title = raw_title.trim();

    if title.is_empty() {
        return Err(AppError::validation_field("title", "Segment name is required"));
    }

    Ok(title.to_string())
}
pub fn slice_builder_route_points(
    route_points: &[ActivityRoutePoint],
    start_route_point_index: i32,
    end_route_point_index: i32,
) -> Result<Vec<ActivityRoutePoint>, AppError> {
    if route_points.len() < 2 {
        return Err(AppError::validation_field(
            "activity_id",
            "Selected activity does not have enough route data to build a segment",
        ));
    }

    let start_index = usize::try_from(start_route_point_index).map_err(|_| {
        AppError::validation_field(
            "start_route_point_index",
            "Segment start must be within the selected activity route",
        )
    })?;
    let end_index = usize::try_from(end_route_point_index).map_err(|_| {
        AppError::validation_field(
            "end_route_point_index",
            "Segment end must be within the selected activity route",
        )
    })?;

    if start_index >= route_points.len() {
        return Err(AppError::validation_field(
            "start_route_point_index",
            "Segment start must be within the selected activity route",
        ));
    }

    if end_index >= route_points.len() {
        return Err(AppError::validation_field(
            "end_route_point_index",
            "Segment end must be within the selected activity route",
        ));
    }

    if start_index >= end_index {
        return Err(AppError::validation_field(
            "start_route_point_index",
            "Segment start must come before the end",
        ));
    }

    Ok(slice_effort_route_points(
        route_points,
        start_route_point_index,
        end_route_point_index,
    ))
}

impl Model {
    pub async fn update_details(
        self,
        db: &impl ConnectionTrait,
        title: Option<&str>,
        mode: Option<SegmentMode>,
        starred: Option<bool>,
    ) -> Result<Self, AppError> {
        let title = match title {
            Some(value) => normalize_segment_title(value)?,
            None => self.title.clone(),
        };
        let mode = mode.unwrap_or_else(|| SegmentMode::from_stored(&self.mode));
        let starred = starred.unwrap_or(self.starred);
        let title_changed = self.title != title;
        let mode_changed = SegmentMode::from_stored(&self.mode) != mode;
        let starred_changed = self.starred != starred;

        let details_changed = title_changed || mode_changed;
        if !details_changed && !starred_changed {
            return Ok(self);
        }

        let mut patch = ActiveModel {
            id: Set(self.id),
            ..Default::default()
        };
        if title_changed {
            patch.title = Set(title);
        }
        if mode_changed {
            patch.mode = Set(mode.as_str().to_string());
        }
        if starred_changed {
            patch.starred = Set(starred);
        }
        Ok(patch.update(db).await?)
    }
    pub async fn create_from_activity(
        db: &impl ConnectionTrait,
        source: &ActivitySegmentSource,
    ) -> Result<segments::Model, AppError> {
        Ok(ActiveModel {
            user_id: Set(source.activity.user_id),
            ..Default::default()
        }
        .with_activity_source(source)
        .insert(db)
        .await?)
    }
    pub async fn import_route(
        db: &impl ConnectionTrait,
        user_id: i32,
        filename: &str,
        format: &str,
        bytes: &[u8],
    ) -> Result<SegmentImport, AppError> {
        let segment_summary = summarize_activity_upload(filename, format, bytes)?;
        let segment_detail = derive_activity_detail_data(filename, format, bytes)?;

        if segment_detail.route_points.len() < 2 {
            return Err(AppError::validation_field(
                "file",
                "Segment imports require GPX or TCX route data with coordinates",
            ));
        }

        if let Some(existing_segment) = segments::Model::find_duplicate(
            db,
            user_id,
            segment_summary.distance_meters,
            &segment_detail.route_points,
        )
        .await?
        {
            return Ok(SegmentImport::Existing(existing_segment));
        }

        let segment = ActiveModel {
            title: Set(segment_summary.title),
            source: Set("manual_segment_import".into()),
            original_filename: Set(Some(filename.to_owned())),
            format: Set(Some(format.to_owned())),
            user_id: Set(user_id),
            ..Default::default()
        }
        .with_route(segment_summary.distance_meters, &segment_detail.route_points)
        .insert(db)
        .await?;

        Ok(SegmentImport::Created(segment))
    }
}

impl ActiveModel {
    fn with_builder_link(self, source: &ActivitySegmentSource) -> Self {
        Self {
            source_activity_id: Set(Some(source.activity.id)),
            source_start_route_point_index: Set(Some(source.start_route_point_index)),
            source_end_route_point_index: Set(Some(source.end_route_point_index)),
            ..self
        }
    }

    fn with_route(self, distance_meters: Option<f64>, route_points: &[ActivityRoutePoint]) -> Self {
        Self {
            distance_meters: Set(distance_meters),
            route_data_json: Set(Some(serialize_segment_route_points(route_points))),
            ..self
        }
    }

    fn with_activity_source(self, source: &ActivitySegmentSource) -> Self {
        Self {
            title: Set(source.title.clone()),
            source: Set("activity_segment_builder".into()),
            original_filename: Set(None),
            format: Set(source.activity.format.clone()),
            ..self.with_builder_link(source)
        }
        .with_route(source.distance_meters, &source.route_points)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[tokio::test]
    async fn unchanged_segment_metadata_does_not_write_or_read_related_results() {
        let original = test_support::segment();
        let db = test_support::postgres_mock().into_connection();
        let updated = original
            .clone()
            .update_details(&db, Some("  Fixture segment  "), None, None)
            .await
            .unwrap();
        assert_eq!(updated, original);
        assert!(db.into_transaction_log().is_empty());
    }

    #[tokio::test]
    async fn blank_segment_names_are_rejected_before_any_write() {
        let db = test_support::postgres_mock().into_connection();
        let error = test_support::segment()
            .update_details(&db, Some("  "), None, None)
            .await
            .unwrap_err();
        assert_eq!(error.errors.unwrap()["title"], vec!["Segment name is required"]);
        assert!(db.into_transaction_log().is_empty());
    }

    #[tokio::test]
    async fn mode_and_star_changes_preserve_the_fixed_route_and_name() {
        let before = test_support::segment();
        let after = Model {
            mode: "dh".into(),
            starred: true,
            ..before.clone()
        };
        let db = test_support::postgres_mock()
            .append_query_results([vec![after.clone()]])
            .into_connection();
        assert_eq!(
            before
                .update_details(&db, None, Some(SegmentMode::Dh), Some(true))
                .await
                .unwrap(),
            after
        );
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 1);
        let assignment = log[0].statements()[0].sql.split(" WHERE ").next().unwrap();
        assert!(!assignment.contains("\"title\" ="));
        assert!(!assignment.contains("\"route_data_json\" ="));
        assert!(!assignment.contains("\"distance_meters\" ="));
        assert!(assignment.contains("\"mode\" ="));
        assert!(assignment.contains("\"starred\" ="));
    }
}
