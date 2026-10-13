use super::Run;
use crate::{
    entities::{integration_events, strava_workflows},
    strava::objects::ObjectRef,
    workflow_error::WorkflowError,
};
use sea_orm::{EntityTrait, TransactionTrait};
use serde::{de::DeserializeOwned, Serialize};
use tracing::Instrument;

impl Run<'_> {
    /// A pending dependency is not a completed or failed stage checkpoint.
    pub(super) async fn stage_when_ready<T, F>(
        &self,
        stage: &str,
        input: &str,
        future: F,
    ) -> Result<Option<T>, WorkflowError>
    where
        T: Serialize + DeserializeOwned + Send + Sync,
        F: std::future::Future<Output = Result<Option<T>, WorkflowError>> + Send,
    {
        if let Some(output) = self.saved_stage(&format!("{stage}:v1:{input}")).await? {
            return Ok(Some(output));
        }
        let output = match future.await {
            Ok(None) => return Ok(None),
            Ok(Some(output)) => Ok(output),
            Err(error) => Err(error),
        };
        self.stage(stage, input, async move { output })
            .await
            .map(Some)
    }

    pub(super) fn stage<'a, T, F>(
        &'a self,
        stage: &'a str,
        input: &'a str,
        future: F,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, WorkflowError>> + Send + 'a>>
    where
        T: Serialize + DeserializeOwned + Send + Sync + 'a,
        F: std::future::Future<Output = Result<T, WorkflowError>> + Send + 'a,
    {
        Box::pin(self.run_stage(
            stage,
            input,
            Box::pin(async move {
                let output = future.await?;
                self.store
                    .transform(&self.stage_prefix(stage, input), &output)
                    .await
            }),
        ))
    }

    pub(super) fn source_stage<'a, T, F>(
        &'a self,
        stage: &'a str,
        input: &'a str,
        future: F,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, WorkflowError>> + Send + 'a>>
    where
        T: Serialize + DeserializeOwned + Send + Sync + 'a,
        F: std::future::Future<Output = Result<ObjectRef, WorkflowError>> + Send + 'a,
    {
        Box::pin(self.run_stage(stage, input, Box::pin(future)))
    }

    async fn run_stage<T>(
        &self,
        stage: &str,
        input: &str,
        future: std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ObjectRef, WorkflowError>> + Send + '_>,
        >,
    ) -> Result<T, WorkflowError>
    where
        T: Serialize + DeserializeOwned + Send + Sync,
    {
        let key = format!("{stage}:v1:{input}");
        let span = tracing::info_span!("strava.stage", stage, input_hash = input, correlation_id = %self.row.correlation_id(), workflow_id = %self.row.id, parent_workflow_id = self.source.parent.as_deref(), strava_activity_id = self.row.activity_id, athlete_id = self.row.athlete_id, trace_id = tracing::field::Empty, span_id = tracing::field::Empty);
        crate::observability::record_span_trace_context(&span);
        let result = async {
            if let Some(output) = self.saved_stage(&key).await? {
                return Ok(output);
            }
            if let Some(output) = self.recover_stage(stage, input, &key).await? {
                return Ok(output);
            }
            let object = future.await?;
            let output: T = self.store.read_json(&object).await?;
            self.store.save_reference(&self.stage_prefix(stage, input), &object).await?;
            self.complete_stage(stage, input, &key, &object, &output).await?;
            tracing::info!(stage, object_path = %object.path, outcome = "completed", "Saved Strava stage output");
            Ok(output)
        }.instrument(span).await;
        match result {
            Ok(output) => Ok(output),
            Err(error) => {
                self.record_stage_failure(stage, input, &error).await?;
                Err(error)
            }
        }
    }

    async fn recover_stage<T: Serialize + DeserializeOwned>(
        &self,
        stage: &str,
        input: &str,
        key: &str,
    ) -> Result<Option<T>, WorkflowError> {
        let prefix = self.stage_prefix(stage, input);
        if let Some(object) = self.store.reference(&prefix).await? {
            let output =
                serde_json::from_slice(&self.store.read(&object).await?).map_err(json_error)?;
            self.complete_stage(stage, input, key, &object, &output)
                .await?;
            tracing::info!(stage, object_path = %object.path, outcome = "reused", "Recovered saved Strava stage");
            return Ok(Some(output));
        }
        Ok(None)
    }

    async fn record_stage_failure(
        &self,
        stage: &str,
        input: &str,
        error: &WorkflowError,
    ) -> Result<(), WorkflowError> {
        tracing::error!(correlation_id = %self.row.correlation_id(), workflow_id = %self.row.id, stage, error = %error.message, retry_at = ?error.retry_at, "Strava stage failed");
        integration_events::Entity::record(self.db, integration_events::NewEvent {
                    user_id: Some(self.connection.user_id),
                    connection_id: Some(self.connection.id),
                    provider: "strava".into(),
                    event_type: "workflow.stage.failed".into(),
                    level: "error".into(),
                    message: format!("Strava {}: {stage} failed: {}", self.row.action, error.message),
                    payload: Some(serde_json::json!({"workflow_id":self.row.id,"correlation_id":self.row.correlation_id(),"parent_workflow_id":self.source.parent,"strava_activity_id":self.row.activity_id,"athlete_id":self.row.athlete_id,"stage":stage,"input_hash":input,"retry_at":error.retry_at,"status_code":error.status.as_u16()})),
                }).await?;
        Ok(())
    }

    fn stage_prefix(&self, stage: &str, input: &str) -> String {
        crate::strava::paths::WorkflowPaths::new(&self.row).transform(stage, input)
    }

    async fn saved_stage<T: DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<Option<T>, WorkflowError> {
        let row = strava_workflows::Entity::find_by_id(&self.row.id)
            .one(self.db)
            .await?
            .ok_or_else(|| WorkflowError::not_found("Strava workflow was removed"))?;
        let Some(saved) = row.checkpoint.get(key) else {
            return Ok(None);
        };
        let object: ObjectRef = serde_json::from_value(saved.clone()).map_err(json_error)?;
        tracing::info!(stage = key, object_path = %object.path, outcome = "reused", "Reusing completed Strava stage");
        let output =
            serde_json::from_slice(&self.store.read(&object).await?).map_err(json_error)?;
        Ok(Some(output))
    }

    async fn complete_stage<T: Serialize>(
        &self,
        stage: &str,
        input: &str,
        key: &str,
        object: &ObjectRef,
        output: &T,
    ) -> Result<(), WorkflowError> {
        let txn = self.db.begin().await?;
        // A stage can commit derived-task intents while executing. Merge into
        // the latest checkpoint so completion never erases their retry guards.
        let row = strava_workflows::Entity::find_by_id(&self.row.id)
            .one(&txn)
            .await?
            .ok_or_else(|| WorkflowError::not_found("Strava workflow was removed"))?;
        let mut checkpoint = row.checkpoint.clone();
        checkpoint[key] = serde_json::to_value(object).map_err(json_error)?;
        row.checkpoint(&txn, checkpoint).await?;
        integration_events::Entity::record(&txn, self.stage_event(stage, input, object, output)?)
            .await?;
        txn.commit().await?;
        if let Some(execution) = crate::background_jobs::execution::ExecutionContext::current() {
            execution.record_progress(self.db).await;
        }
        Ok(())
    }

    fn stage_event<T: Serialize>(
        &self,
        stage: &str,
        input: &str,
        object: &ObjectRef,
        output: &T,
    ) -> Result<integration_events::NewEvent, WorkflowError> {
        let output = serde_json::to_value(output).map_err(json_error)?;
        let outcome = output
            .get("outcome")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("completed")
            .to_owned();
        Ok(integration_events::NewEvent {
            user_id: Some(self.connection.user_id),
            connection_id: Some(self.connection.id),
            provider: "strava".into(),
            event_type: "workflow.stage".into(),
            level: "info".into(),
            message: format!("Strava {}: {stage} {outcome}.", self.row.action),
            payload: Some(
                serde_json::json!({"workflow_id":self.row.id,"correlation_id":self.row.correlation_id(),"parent_workflow_id":self.source.parent,"strava_activity_id":self.row.activity_id,"athlete_id":self.row.athlete_id,"stage":stage,"activity_id":output.get("activity_id"),"import_id":output.get("import_id"),"action":self.row.action,"input_hash":input,"object":object,"outcome":outcome}),
            ),
        })
    }
}
pub(super) fn json_error(error: serde_json::Error) -> WorkflowError {
    WorkflowError::internal(error.to_string())
}
