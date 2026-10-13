use super::{inbox, Action, Run, Source};
use crate::{
    background_jobs::background_tasks, entities::strava_workflows, workflow_error::WorkflowError,
};
use sea_orm::TransactionTrait;

impl Run<'_> {
    /// Acceptance and the worker handoff commit together; never execute inline.
    pub(super) async fn child_activity(
        &self,
        id: &str,
        action: Action,
        activity_id: i64,
        source: Source,
    ) -> Result<strava_workflows::Model, WorkflowError> {
        let transaction = self.db.begin().await?;
        let inserted = inbox::accept(
            &transaction,
            &self.connection,
            id,
            action,
            Some(activity_id),
            source,
        )
        .await?;
        let row = strava_workflows::Model::load(&transaction, id)
            .await?
            .ok_or_else(|| WorkflowError::internal("Strava child workflow was not retained"))?;
        let task = background_tasks::Model::latest_strava_workflow_task(&transaction, id).await?;
        // Older inline children can have a retained workflow without a worker task.
        if inserted || (row.status != "completed" && task.is_none()) {
            inbox::enqueue(&transaction, id).await?;
        }
        transaction.commit().await?;
        Ok(row)
    }

    pub(super) async fn child_completed(
        &self,
        row: &strava_workflows::Model,
    ) -> Result<bool, WorkflowError> {
        let task = background_tasks::Model::latest_strava_workflow_task(self.db, &row.id).await?;
        match task {
            Some(task) if matches!(task.status.as_str(), "failed" | "canceled") => {
                Err(child_failure(&task))
            }
            Some(task) if task.status == "completed" && row.status == "completed" => Ok(true),
            Some(task) if matches!(task.status.as_str(), "pending" | "processing") => Ok(false),
            Some(task) => Err(child_failure(&task)),
            None if row.status == "completed" => Ok(true),
            None => Err(WorkflowError::internal("Strava child has no worker task")),
        }
    }
}

fn child_failure(task: &background_tasks::Model) -> WorkflowError {
    WorkflowError::internal(format!(
        "Strava activity task {} did not complete its workflow: {}",
        task.id,
        task.error.as_deref().unwrap_or(task.status.as_str())
    ))
}
