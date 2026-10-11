use crate::{strava_objects::ObjectStore, workflow_error::WorkflowError};
use serde::Serialize;

#[derive(Clone)]
pub(crate) struct ImportObjects {
    pub store: ObjectStore,
    pub athlete_id: i64,
    pub activity_id: i64,
}
tokio::task_local! { static CURRENT: ImportObjects; }
impl ImportObjects {
    pub(crate) async fn scope<F: std::future::Future>(self, future: F) -> F::Output {
        CURRENT.scope(self, future).await
    }
}

pub(crate) async fn save<T: Serialize>(
    stage: &str,
    input: &str,
    value: &T,
) -> Result<(), WorkflowError> {
    if let Ok(objects) = CURRENT.try_with(Clone::clone) {
        let prefix = format!(
            "strava/{}/{}/transforms/import-{stage}/v1/{input}",
            objects.athlete_id, objects.activity_id
        );
        let object = objects.store.transform(&prefix, value).await?;
        tracing::info!(strava_activity_id = objects.activity_id, athlete_id = objects.athlete_id, stage, object_path = %object.path, "Retained import DAG transform");
    }
    Ok(())
}
