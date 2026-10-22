use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("activity_imports"))
                    .add_column(ColumnDef::new(Alias::new("archive_job_id")).integer())
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Attempts::Table)
                    .col(
                        ColumnDef::new(Attempts::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Attempts::UserId).integer().not_null())
                    .col(ColumnDef::new(Attempts::ActivityImportId).integer().not_null())
                    .col(ColumnDef::new(Attempts::ActivityId).integer())
                    .col(ColumnDef::new(Attempts::Status).string().not_null())
                    .col(ColumnDef::new(Attempts::RequestedStage).string().not_null())
                    .col(ColumnDef::new(Attempts::StartStage).string().not_null())
                    .col(ColumnDef::new(Attempts::CurrentStage).string().not_null())
                    .col(ColumnDef::new(Attempts::ReusedAttemptId).integer())
                    .col(ColumnDef::new(Attempts::SourceJson).json_binary().not_null())
                    .col(ColumnDef::new(Attempts::StagesJson).json_binary().not_null())
                    .col(ColumnDef::new(Attempts::CheckpointJson).json_binary())
                    .col(ColumnDef::new(Attempts::Error).text())
                    .col(
                        ColumnDef::new(Attempts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Attempts::StartedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Attempts::FinishedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(Attempts::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Attempts::Table, Attempts::ActivityImportId)
                            .to(Alias::new("activity_imports"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("activity_import_attempt_history")
                    .table(Attempts::Table)
                    .col(Attempts::UserId)
                    .col(Attempts::ActivityImportId)
                    .col(Attempts::Id)
                    .to_owned(),
            )
            .await?;
        // Partial unique indexes enforce one live execution across workers and API requests.
        manager.get_connection().execute_unprepared(
            "CREATE UNIQUE INDEX activity_import_attempt_active_import ON activity_import_attempts (activity_import_id) WHERE status IN ('queued', 'running')",
        ).await?;
        manager.get_connection().execute_unprepared(
            "CREATE UNIQUE INDEX activity_import_attempt_active_activity ON activity_import_attempts (activity_id) WHERE status IN ('queued', 'running') AND activity_id IS NOT NULL",
        ).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Attempts::Table).to_owned())
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("activity_imports"))
                    .drop_column(Alias::new("archive_job_id"))
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Attempts {
    #[sea_orm(iden = "activity_import_attempts")]
    Table,
    Id,
    UserId,
    ActivityImportId,
    ActivityId,
    Status,
    RequestedStage,
    StartStage,
    CurrentStage,
    ReusedAttemptId,
    SourceJson,
    StagesJson,
    CheckpointJson,
    Error,
    CreatedAt,
    StartedAt,
    FinishedAt,
    UpdatedAt,
}
