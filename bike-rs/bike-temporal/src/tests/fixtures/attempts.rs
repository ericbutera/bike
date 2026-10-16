use super::*;

#[derive(Default)]
pub(crate) struct MockImportAttempts {
    pub recorded: Mutex<Vec<ImportExecution>>,
    pub completed: Mutex<Vec<(i32, ActivityImportResult)>>,
    pub fail_record: bool,
    pub fail_complete: bool,
}

#[async_trait::async_trait]
impl ImportAttemptRepository for Operations {
    async fn record(&self, execution: &ImportExecution) -> Result<i32, WorkflowError> {
        self.attempts
            .recorded
            .lock()
            .unwrap()
            .push(execution.clone());
        if self.attempts.fail_record {
            return Err(WorkflowError::internal("fixture recording failed"));
        }
        Ok(fixtures::activity_import_attempt().id)
    }

    async fn complete(
        &self,
        attempt_id: i32,
        result: &ActivityImportResult,
    ) -> Result<(), WorkflowError> {
        self.attempts
            .completed
            .lock()
            .unwrap()
            .push((attempt_id, result.clone()));
        if self.attempts.fail_complete {
            return Err(WorkflowError::internal(
                "fixture completion recording failed",
            ));
        }
        Ok(())
    }
}
