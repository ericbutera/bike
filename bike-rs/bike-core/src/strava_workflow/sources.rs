use super::{Run, Source};
use crate::{
    entities::strava_connections,
    strava_client::RequestArchive,
    strava_objects::{ObjectRef, ObjectStore},
    workflow_error::WorkflowError,
};
use sea_orm::EntityTrait;

impl Source {
    pub(super) async fn activity_summary(
        &self,
        store: &ObjectStore,
        id: i64,
    ) -> Result<Option<serde_json::Value>, WorkflowError> {
        let Some(page) = &self.listing_page else {
            // Work accepted before object-backed listing sources remains resumable.
            return Ok(self.activity.clone());
        };
        let values: Vec<serde_json::Value> = store.read_json(page).await?;
        values
            .into_iter()
            .find(|value| value["id"].as_i64() == Some(id))
            .map(Some)
            .ok_or_else(|| WorkflowError::internal("Saved Strava listing has no matching activity"))
    }
}

impl Run<'_> {
    /// Raw inputs are authoritative. Credentials and quota are needed only on a miss.
    pub(super) async fn fetch_source<T, F, Fut>(
        &self,
        archive: RequestArchive,
        request: F,
    ) -> Result<ObjectRef, WorkflowError>
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<T, WorkflowError>>,
    {
        if let Some(saved) = archive.store.response(&archive.key).await? {
            tracing::info!(object_path = %saved.body.path, outcome = "reused", "Reusing saved Strava source");
            return Ok(saved.body);
        }
        let token = self.access_token().await?;
        archive.clone().scope(request(token)).await?;
        archive
            .store
            .response(&archive.key)
            .await?
            .map(|saved| saved.body)
            .ok_or_else(|| WorkflowError::internal("Strava response was not retained"))
    }

    pub(super) async fn access_token(&self) -> Result<String, WorkflowError> {
        if !super::provider_work_enabled() {
            return Err(WorkflowError::too_many_requests(
                "Strava provider work is paused for cutover",
                Some(chrono::Utc::now() + chrono::Duration::minutes(1)),
            ));
        }
        // Child stages share the connection lease, but may have refreshed it already.
        let connection = strava_connections::Entity::find_by_id(self.connection.id)
            .one(self.db)
            .await?
            .filter(|connection| connection.created_at == self.source.generation)
            .ok_or_else(|| WorkflowError::conflict("Strava connection changed"))?;
        let connection =
            crate::strava::ensure_fresh_access_token(self.db, &self.client, connection).await?;
        crate::strava_credentials::open(&connection.access_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn listing_sources_select_retained_activity_and_reject_missing_identity() {
        let files = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(files.path());
        let values = json!([{"id":1,"name":"other"},{"id":2,"unknown_vendor_field":true}]);
        let page = store
            .transform("strava/athlete/runs/run/raw/pages/1", &values)
            .await
            .unwrap();
        let source: Source = serde_json::from_value(
            json!({"generation":chrono::Utc::now(),"event_time":1,"listing_page":page}),
        )
        .unwrap();
        assert_eq!(
            source.activity_summary(&store, 2).await.unwrap(),
            Some(values[1].clone())
        );
        assert!(source
            .activity_summary(&store, 3)
            .await
            .unwrap_err()
            .message
            .contains("no matching activity"));
    }

    #[tokio::test]
    async fn legacy_inline_activity_sources_remain_readable() {
        let files = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(files.path());
        let activity = json!({"id":2,"unknown_vendor_field":true});
        let source: Source = serde_json::from_value(
            json!({"generation":chrono::Utc::now(),"event_time":1,"activity":activity}),
        )
        .unwrap();
        assert_eq!(
            source.activity_summary(&store, 2).await.unwrap(),
            Some(activity)
        );
    }
}
