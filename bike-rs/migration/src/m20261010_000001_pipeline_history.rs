use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(runs()).await?;
        manager.create_table(tasks()).await?;
        manager.create_table(attempts()).await?;
        for (name, table, columns) in [
            (
                "pipeline_runs_trace",
                "processing_pipeline_runs",
                vec!["trace_id"],
            ),
            (
                "task_attempts_trace",
                "background_task_attempts",
                vec!["trace_id"],
            ),
            (
                "pipeline_runs_received",
                "processing_pipeline_runs",
                vec!["pipeline_started_at"],
            ),
            (
                "pipeline_runs_request",
                "processing_pipeline_runs",
                vec!["request_id"],
            ),
            (
                "pipeline_tasks_task",
                "processing_pipeline_tasks",
                vec!["task_id"],
            ),
            (
                "task_attempts_running",
                "background_task_attempts",
                vec!["outcome", "started_at"],
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
        for table in [
            "background_task_attempts",
            "processing_pipeline_tasks",
            "processing_pipeline_runs",
        ] {
            manager
                .drop_table(Table::drop().table(Alias::new(table)).to_owned())
                .await?;
        }
        Ok(())
    }
}

fn runs() -> TableCreateStatement {
    Table::create()
        .table(Alias::new("processing_pipeline_runs"))
        .col(
            ColumnDef::new(Alias::new("id"))
                .string_len(36)
                .not_null()
                .primary_key(),
        )
        .col(
            ColumnDef::new(Alias::new("pipeline_started_at"))
                .timestamp_with_time_zone()
                .not_null(),
        )
        .col(
            ColumnDef::new(Alias::new("accepted_at"))
                .timestamp_with_time_zone()
                .not_null(),
        )
        .col(ColumnDef::new(Alias::new("entrypoint")).string().not_null())
        .col(ColumnDef::new(Alias::new("request_id")).string())
        .col(ColumnDef::new(Alias::new("trace_id")).string_len(32))
        .col(ColumnDef::new(Alias::new("available_at")).timestamp_with_time_zone())
        .to_owned()
}

fn tasks() -> TableCreateStatement {
    Table::create()
        .table(Alias::new("processing_pipeline_tasks"))
        .col(
            ColumnDef::new(Alias::new("run_id"))
                .string_len(36)
                .not_null(),
        )
        .col(ColumnDef::new(Alias::new("task_id")).integer().not_null())
        .col(ColumnDef::new(Alias::new("parent_task_id")).integer())
        .primary_key(
            Index::create()
                .col(Alias::new("run_id"))
                .col(Alias::new("task_id")),
        )
        .foreign_key(
            ForeignKey::create()
                .from(
                    Alias::new("processing_pipeline_tasks"),
                    Alias::new("run_id"),
                )
                .to(Alias::new("processing_pipeline_runs"), Alias::new("id"))
                .on_delete(ForeignKeyAction::Cascade),
        )
        .foreign_key(&mut task_reference("processing_pipeline_tasks", "task_id"))
        .to_owned()
}

fn attempts() -> TableCreateStatement {
    Table::create()
        .table(Alias::new("background_task_attempts"))
        .col(ColumnDef::new(Alias::new("task_id")).integer().not_null())
        .col(ColumnDef::new(Alias::new("attempt")).integer().not_null())
        .col(
            ColumnDef::new(Alias::new("started_at"))
                .timestamp_with_time_zone()
                .not_null(),
        )
        .col(ColumnDef::new(Alias::new("finished_at")).timestamp_with_time_zone())
        .col(
            ColumnDef::new(Alias::new("heartbeat_at"))
                .timestamp_with_time_zone()
                .not_null(),
        )
        .col(
            ColumnDef::new(Alias::new("progress_at"))
                .timestamp_with_time_zone()
                .not_null(),
        )
        .col(ColumnDef::new(Alias::new("outcome")).string().not_null())
        .col(ColumnDef::new(Alias::new("error")).text())
        .col(ColumnDef::new(Alias::new("trace_id")).string_len(32))
        .col(ColumnDef::new(Alias::new("span_id")).string_len(16))
        .primary_key(
            Index::create()
                .col(Alias::new("task_id"))
                .col(Alias::new("attempt")),
        )
        .foreign_key(&mut task_reference("background_task_attempts", "task_id"))
        .to_owned()
}

fn task_reference(table: &str, column: &str) -> ForeignKeyCreateStatement {
    ForeignKey::create()
        .from(Alias::new(table), Alias::new(column))
        .to(Alias::new("background_tasks"), Alias::new("id"))
        .on_delete(ForeignKeyAction::Cascade)
        .to_owned()
}
