use async_trait::async_trait;
use bike_core::{
    bike_tasks::{ActivityImport, ActivityImportResult, ImportExecution, JobReference},
    entities::activity_import_attempts,
    workflow_error::WorkflowError,
};
use sea_orm::DatabaseConnection;
use temporalio_sdk::activities::ActivityContext;

#[async_trait]
pub trait ImportAttemptRepository: Send + Sync {
    async fn record(&self, execution: &ImportExecution) -> Result<i32, WorkflowError>;
    async fn complete(
        &self,
        attempt_id: i32,
        result: &ActivityImportResult,
    ) -> Result<(), WorkflowError>;
}

pub struct DatabaseImportAttempts {
    db: DatabaseConnection,
}

impl DatabaseImportAttempts {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ImportAttemptRepository for DatabaseImportAttempts {
    async fn record(&self, execution: &ImportExecution) -> Result<i32, WorkflowError> {
        Ok(
            activity_import_attempts::Model::record_temporal(&self.db, execution)
                .await?
                .id,
        )
    }

    async fn complete(
        &self,
        attempt_id: i32,
        result: &ActivityImportResult,
    ) -> Result<(), WorkflowError> {
        activity_import_attempts::Model::complete_temporal(&self.db, attempt_id, result).await
    }
}

pub(super) fn execution(
    ctx: &ActivityContext,
    command: ActivityImport,
) -> Result<ImportExecution, WorkflowError> {
    let info = ctx.info();
    let required = |value: &Option<String>| {
        value.clone().ok_or_else(|| {
            WorkflowError::bad_request("Import requires a Temporal workflow execution")
        })
    };
    Ok(ImportExecution {
        command,
        workflow: JobReference {
            namespace: info.namespace.clone(),
            job_id: required(&info.workflow_id)?,
            run_id: required(&info.workflow_run_id)?,
            workflow_type: required(&info.workflow_type)?,
        },
    })
}
