use super::{ActiveModel, Model, SegmentMode};
use crate::activity_details::{
    derive_activity_detail_data, deserialize_derived_activity_data, ActivityRoutePoint,
};
use crate::activity_summary::summarize_activity_upload;
use crate::analytics::rebuild_activity_analytics_cache;
use crate::entities::{activities, segment_efforts, segments};
use crate::segment_support::{serialize_segment_route_points, slice_effort_route_points};
use crate::workflow_error::WorkflowError as AppError;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, IntoActiveModel, Set, TransactionSession, TransactionTrait,
};

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
    fn require_owner(&self, user_id: i32) -> Result<(), AppError> {
        if self.activity.user_id != user_id {
            return Err(AppError::not_found("Activity not found"));
        }
        Ok(())
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
        let activity_route_points =
            deserialize_derived_activity_data(activity.derived_data_json.as_ref()).route_points;
        let route_points = slice_builder_route_points(
            &activity_route_points,
            start_route_point_index,
            end_route_point_index,
        )?;
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
    fn matches_builder(&self, segment: &Model) -> bool {
        let activity_matches = segment.source_activity_id == Some(self.activity.id);
        let range_matches = (
            segment.source_start_route_point_index,
            segment.source_end_route_point_index,
        ) == (
            Some(self.start_route_point_index),
            Some(self.end_route_point_index),
        );
        activity_matches && range_matches
    }
}

pub fn normalize_segment_title(raw_title: &str) -> Result<String, AppError> {
    let title = raw_title.trim();

    if title.is_empty() {
        return Err(AppError::validation_field(
            "title",
            "Segment name is required",
        ));
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
        db: &(impl ConnectionTrait + TransactionTrait),
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

        let activity_ids = if title_changed {
            segment_efforts::Model::activity_ids_for_segments(db, &[self.id]).await?
        } else {
            Vec::new()
        };
        let txn = db.begin().await?;
        let mut active_segment = self.into_active_model();
        active_segment.title = Set(title);
        active_segment.mode = Set(mode.as_str().to_string());
        active_segment.starred = Set(starred);
        let updated_segment = active_segment.update(&txn).await?;

        if title_changed && !activity_ids.is_empty() {
            rebuild_activity_analytics_cache(&txn, &activity_ids).await?;
        }

        txn.commit().await?;

        Ok(updated_segment)
    }
    pub async fn adopt_builder_source(
        self,
        db: &(impl ConnectionTrait + TransactionTrait),
        source: &ActivitySegmentSource,
    ) -> Result<Self, AppError> {
        source.require_owner(self.user_id)?;
        let should_update_title = self.title != source.title;
        let should_update_builder_source = !source.matches_builder(&self);

        if !should_update_title && !should_update_builder_source {
            return Ok(self);
        }

        let activity_ids = if should_update_title {
            segment_efforts::Model::activity_ids_for_segments(db, &[self.id]).await?
        } else {
            Vec::new()
        };
        let txn = db.begin().await?;
        let mut active_segment = self.into_active_model().with_builder_link(source);
        if should_update_title {
            active_segment.title = Set(source.title.clone());
        }

        let updated_segment = active_segment.update(&txn).await?;

        if !activity_ids.is_empty() {
            rebuild_activity_analytics_cache(&txn, &activity_ids).await?;
        }

        txn.commit().await?;

        Ok(updated_segment)
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
    pub async fn replace_from_activity(
        self,
        db: &(impl ConnectionTrait + TransactionTrait),
        source: &ActivitySegmentSource,
    ) -> Result<Self, AppError> {
        source.require_owner(self.user_id)?;
        if let Some(existing) = Self::find_duplicate(
            db,
            self.user_id,
            source.distance_meters,
            &source.route_points,
        )
        .await?
        {
            if existing.id != self.id {
                return Err(AppError::validation_field(
                    "activity_id",
                    "Another segment already uses this route",
                ));
            }
        }
        let txn = db.begin().await?;
        let updated = self
            .into_active_model()
            .with_activity_source(source)
            .update(&txn)
            .await?;
        txn.commit().await?;
        Ok(updated)
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
        .with_route(
            segment_summary.distance_meters,
            &segment_detail.route_points,
        )
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
