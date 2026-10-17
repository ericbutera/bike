use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Stop and drain the old worker before applying this release. Source files,
        // import metadata and activity event history remain available for reruns.
        for table in [
            "worker_batch_tasks",
            "worker_batches",
            "processing_work_units",
            "worker_task_anomalies",
            "processing_pipeline_outputs",
            "processing_pipeline_subjects",
            "processing_pipeline_tasks",
            "background_task_attempts",
            "processing_pipeline_runs",
            "worker_processor_registry",
            "activity_import_attempts",
            "activity_import_locks",
            "background_tasks",
        ] {
            manager
                .drop_table(Table::drop().if_exists().table(Alias::new(table)).to_owned())
                .await?;
        }
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom("SQL worker retirement is forward-only".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult};

    #[tokio::test]
    async fn retirement_drops_execution_tables_and_preserves_domain_data() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_exec_results((0..32).map(|_| MockExecResult {
                rows_affected: 0,
                last_insert_id: 0,
            }))
            .into_connection();
        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();
        assert!(Migration
            .down(&manager)
            .await
            .unwrap_err()
            .to_string()
            .contains("forward-only"));
        let log = db
            .into_transaction_log()
            .iter()
            .flat_map(|transaction| transaction.statements())
            .map(|statement| statement.sql.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(log.contains("background_tasks"));
        for table in [
            "activities",
            "activity_imports",
            "activity_import_artifacts",
            "activity_event_history",
        ] {
            assert!(!log.contains(&format!("\"{table}\"")));
        }
        assert!(!log.contains("CASCADE"));
    }
}
