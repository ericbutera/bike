use crate::entities::heatmap_projections;

pub fn heatmap_projection() -> heatmap_projections::Model {
    heatmap_projections::Model {
        activity_id: 42,
        user_id: 7,
        generation: 3,
        status: "pending".into(),
        projection_version: crate::heatmaps::PROJECTION_VERSION,
        queued_at: None,
        published_at: None,
        error: None,
        min_x: None,
        min_y: None,
        max_x: None,
        max_y: None,
    }
}
