use api::analytics::{
    mark_segment_activity_changes, rebuild_activity_analytics_cache,
    rebuild_segment_analytics_cache,
};
use api::entities::segments;
use api::segment_support::{deserialize_segment_route_points, replace_segment_efforts_for_segment};
use api::tasks::RegenerateSegmentEffortsTask;
use async_trait::async_trait;
use chrono::Utc;
use kaleido::background_jobs::worker::TaskProcessor;
use sea_orm::{DatabaseConnection, EntityTrait};
use std::error::Error;

pub struct RegenerateSegmentEfforts {
    db: DatabaseConnection,
}

impl RegenerateSegmentEfforts {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TaskProcessor for RegenerateSegmentEfforts {
    fn task_type(&self) -> &str {
        "regenerate_segment_efforts"
    }

    async fn process(
        &self,
        _task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: RegenerateSegmentEffortsTask = serde_json::from_value(data.clone())?;
        let segment = segments::Entity::find_by_id(task.segment_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("segment {} was not found", task.segment_id),
                )
            })?;
        let route_points = deserialize_segment_route_points(segment.route_data_json.as_ref());

        let affected_activity_ids = replace_segment_efforts_for_segment(
            &self.db,
            segment.user_id,
            segment.id,
            &route_points,
        )
        .await
        .map_err(|error| std::io::Error::other(error.message))?;
        mark_segment_activity_changes(&self.db, &[segment.id], Utc::now()).await?;
        rebuild_segment_analytics_cache(&self.db, &[segment.id]).await?;
        rebuild_activity_analytics_cache(&self.db, &affected_activity_ids).await?;

        Ok(())
    }
}
