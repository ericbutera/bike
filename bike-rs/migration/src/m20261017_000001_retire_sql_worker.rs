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
