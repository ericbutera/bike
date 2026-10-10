use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("activity_import_attempts"))
                    .add_column(ColumnDef::new(Alias::new("worker_task_id")).integer())
                    .to_owned(),
            )
            .await?;
        for statement in [
            registry(),
            subjects(),
            outputs(),
            work_units(),
            anomalies(),
            batches(),
            batch_tasks(),
        ] {
            manager.create_table(statement).await?;
        }
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("strava_delivery_intents"))
                    .col(
                        ColumnDef::new(Alias::new("delivery_id"))
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("content_hash"))
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("source")).json_binary())
                    .col(
                        ColumnDef::new(Alias::new("accepted_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("completed_at")).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("heatmap_projections"))
                    .add_column(
                        ColumnDef::new(Alias::new("published_at")).timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("background_task_attempts"))
                    .add_column(
                        ColumnDef::new(Alias::new("eligible_at")).timestamp_with_time_zone(),
                    )
                    .add_column(ColumnDef::new(Alias::new("processing_version")).string())
                    .add_column(ColumnDef::new(Alias::new("workload_cohort")).string())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("processing_pipeline_runs"))
                    .add_column(ColumnDef::new(Alias::new("gateway_history")).json_binary())
                    .to_owned(),
            )
            .await?;
        for (name, table, columns) in [
            (
                "pipeline_import_worker",
                "activity_import_attempts",
                vec!["worker_task_id"],
            ),
            (
                "pipeline_attempt_baseline",
                "background_task_attempts",
                vec![
                    "processing_version",
                    "workload_cohort",
                    "outcome",
                    "started_at",
                ],
            ),
            (
                "pipeline_batch_status",
                "worker_batches",
                vec!["status", "updated_at"],
            ),
            (
                "pipeline_subject_lookup",
                "processing_pipeline_subjects",
                vec!["kind", "subject_id"],
            ),
            (
                "pipeline_output_wait",
                "processing_pipeline_outputs",
                vec!["kind", "target_id", "status"],
            ),
            (
                "pipeline_work_attempt",
                "processing_work_units",
                vec!["task_id", "attempt"],
            ),
            (
                "pipeline_work_key",
                "processing_work_units",
                vec!["kind", "work_key", "revision", "started_at"],
            ),
            (
                "pipeline_anomaly_recent",
                "worker_task_anomalies",
                vec!["evaluated_at"],
            ),
        ] {
            let mut index = Index::create();
            index.name(name).table(Alias::new(table));
            for column in columns {
                index.col(Alias::new(column));
            }
            manager.create_index(index.to_owned()).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("activity_import_attempts"))
                    .drop_column(Alias::new("worker_task_id"))
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("strava_delivery_intents"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("heatmap_projections"))
                    .drop_column(Alias::new("published_at"))
                    .to_owned(),
            )
            .await?;
        for table in [
            "worker_batch_tasks",
            "worker_batches",
            "worker_task_anomalies",
            "processing_work_units",
            "processing_pipeline_outputs",
            "processing_pipeline_subjects",
            "worker_processor_registry",
        ] {
            manager
                .drop_table(Table::drop().table(Alias::new(table)).to_owned())
                .await?;
        }
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("background_task_attempts"))
                    .drop_column(Alias::new("eligible_at"))
                    .drop_column(Alias::new("processing_version"))
                    .drop_column(Alias::new("workload_cohort"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("processing_pipeline_runs"))
                    .drop_column(Alias::new("gateway_history"))
                    .to_owned(),
            )
            .await
    }
}

fn batches() -> TableCreateStatement {
    Table::create()
        .table(Alias::new("worker_batches"))
        .col(
            ColumnDef::new(Alias::new("id"))
                .integer()
                .not_null()
                .primary_key(),
        )
        .col(ColumnDef::new(Alias::new("user_id")).integer().not_null())
        .col(ColumnDef::new(Alias::new("kind")).string().not_null())
        .col(
            ColumnDef::new(Alias::new("source"))
                .json_binary()
                .not_null(),
        )
        .col(ColumnDef::new(Alias::new("cursor")).integer().not_null())
        .col(ColumnDef::new(Alias::new("sealed")).boolean().not_null())
        .col(ColumnDef::new(Alias::new("status")).string().not_null())
        .col(
            ColumnDef::new(Alias::new("updated_at"))
                .timestamp_with_time_zone()
                .not_null(),
        )
        .to_owned()
}
fn batch_tasks() -> TableCreateStatement {
    Table::create()
        .table(Alias::new("worker_batch_tasks"))
        .col(ColumnDef::new(Alias::new("batch_id")).integer().not_null())
        .col(ColumnDef::new(Alias::new("task_id")).integer().not_null())
        .primary_key(
            Index::create()
                .col(Alias::new("batch_id"))
                .col(Alias::new("task_id")),
        )
        .foreign_key(
            ForeignKey::create()
                .from(Alias::new("worker_batch_tasks"), Alias::new("batch_id"))
                .to(Alias::new("worker_batches"), Alias::new("id"))
                .on_delete(ForeignKeyAction::Cascade),
        )
        .foreign_key(
            ForeignKey::create()
                .from(Alias::new("worker_batch_tasks"), Alias::new("task_id"))
                .to(Alias::new("background_tasks"), Alias::new("id"))
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned()
}

fn registry() -> TableCreateStatement {
    Table::create()
        .table(Alias::new("worker_processor_registry"))
        .col(
            ColumnDef::new(Alias::new("task_type"))
                .string()
                .not_null()
                .primary_key(),
        )
        .col(
            ColumnDef::new(Alias::new("definition"))
                .json_binary()
                .not_null(),
        )
        .col(
            ColumnDef::new(Alias::new("observed_at"))
                .timestamp_with_time_zone()
                .not_null(),
        )
        .to_owned()
}

fn subjects() -> TableCreateStatement {
    Table::create()
        .table(Alias::new("processing_pipeline_subjects"))
        .col(
            ColumnDef::new(Alias::new("run_id"))
                .string_len(36)
                .not_null(),
        )
        .col(ColumnDef::new(Alias::new("kind")).string().not_null())
        .col(
            ColumnDef::new(Alias::new("subject_id"))
                .integer()
                .not_null(),
        )
        .primary_key(
            Index::create()
                .col(Alias::new("run_id"))
                .col(Alias::new("kind"))
                .col(Alias::new("subject_id")),
        )
        .foreign_key(&mut run_reference("processing_pipeline_subjects"))
        .to_owned()
}

fn outputs() -> TableCreateStatement {
    Table::create()
        .table(Alias::new("processing_pipeline_outputs"))
        .col(
            ColumnDef::new(Alias::new("run_id"))
                .string_len(36)
                .not_null(),
        )
        .col(ColumnDef::new(Alias::new("kind")).string().not_null())
        .col(ColumnDef::new(Alias::new("target_id")).integer().not_null())
        .col(ColumnDef::new(Alias::new("revision")).string().not_null())
        .col(ColumnDef::new(Alias::new("required")).boolean().not_null())
        .col(ColumnDef::new(Alias::new("status")).string().not_null())
        .col(ColumnDef::new(Alias::new("reason")).string())
        .col(
            ColumnDef::new(Alias::new("required_at"))
                .timestamp_with_time_zone()
                .not_null(),
        )
        .col(ColumnDef::new(Alias::new("available_at")).timestamp_with_time_zone())
        .primary_key(
            Index::create()
                .col(Alias::new("run_id"))
                .col(Alias::new("kind"))
                .col(Alias::new("target_id")),
        )
        .foreign_key(&mut run_reference("processing_pipeline_outputs"))
        .to_owned()
}

fn work_units() -> TableCreateStatement {
    Table::create()
        .table(Alias::new("processing_work_units"))
        .col(
            ColumnDef::new(Alias::new("id"))
                .integer()
                .not_null()
                .auto_increment()
                .primary_key(),
        )
        .col(ColumnDef::new(Alias::new("task_id")).integer().not_null())
        .col(ColumnDef::new(Alias::new("attempt")).integer().not_null())
        .col(ColumnDef::new(Alias::new("kind")).string().not_null())
        .col(ColumnDef::new(Alias::new("work_key")).string().not_null())
        .col(ColumnDef::new(Alias::new("revision")).string().not_null())
        .col(
            ColumnDef::new(Alias::new("processing_version"))
                .string()
                .not_null(),
        )
        .col(ColumnDef::new(Alias::new("mode")).string().not_null())
        .col(ColumnDef::new(Alias::new("reason")).string().not_null())
        .col(
            ColumnDef::new(Alias::new("started_at"))
                .timestamp_with_time_zone()
                .not_null(),
        )
        .col(
            ColumnDef::new(Alias::new("finished_at"))
                .timestamp_with_time_zone()
                .not_null(),
        )
        .col(ColumnDef::new(Alias::new("outcome")).string().not_null())
        .col(ColumnDef::new(Alias::new("counts")).json_binary())
        .col(ColumnDef::new(Alias::new("error")).text())
        .col(ColumnDef::new(Alias::new("trace_id")).string_len(32))
        .col(ColumnDef::new(Alias::new("span_id")).string_len(16))
        .foreign_key(
            ForeignKey::create()
                .from(Alias::new("processing_work_units"), Alias::new("task_id"))
                .to(Alias::new("background_tasks"), Alias::new("id"))
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned()
}

fn anomalies() -> TableCreateStatement {
    Table::create()
        .table(Alias::new("worker_task_anomalies"))
        .col(ColumnDef::new(Alias::new("task_id")).integer().not_null())
        .col(ColumnDef::new(Alias::new("attempt")).integer().not_null())
        .col(ColumnDef::new(Alias::new("reason")).string().not_null())
        .col(
            ColumnDef::new(Alias::new("policy_version"))
                .integer()
                .not_null(),
        )
        .col(ColumnDef::new(Alias::new("observed")).double().not_null())
        .col(ColumnDef::new(Alias::new("expected")).double().not_null())
        .col(
            ColumnDef::new(Alias::new("baseline_samples"))
                .big_integer()
                .not_null(),
        )
        .col(
            ColumnDef::new(Alias::new("evaluated_at"))
                .timestamp_with_time_zone()
                .not_null(),
        )
        .col(ColumnDef::new(Alias::new("resolved_at")).timestamp_with_time_zone())
        .col(ColumnDef::new(Alias::new("severity")).string().not_null())
        .primary_key(
            Index::create()
                .col(Alias::new("task_id"))
                .col(Alias::new("attempt"))
                .col(Alias::new("reason"))
                .col(Alias::new("policy_version")),
        )
        .foreign_key(
            ForeignKey::create()
                .from(Alias::new("worker_task_anomalies"), Alias::new("task_id"))
                .to(Alias::new("background_tasks"), Alias::new("id"))
                .on_delete(ForeignKeyAction::Cascade),
        )
        .to_owned()
}

fn run_reference(table: &str) -> ForeignKeyCreateStatement {
    ForeignKey::create()
        .from(Alias::new(table), Alias::new("run_id"))
        .to(Alias::new("processing_pipeline_runs"), Alias::new("id"))
        .on_delete(ForeignKeyAction::Cascade)
        .to_owned()
}
