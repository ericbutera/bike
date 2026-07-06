use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum ActivityImports {
    Table,
    UserId,
    Status,
    ActivityId,
    ProcessingStage,
    ProcessingError,
    ProcessingAttempts,
    ProcessedAt,
    LastProcessingEventAt,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ActivityImports::Table)
                    .add_column(ColumnDef::new(ActivityImports::ActivityId).integer().null())
                    .add_column(
                        ColumnDef::new(ActivityImports::ProcessingStage)
                            .string_len(64)
                            .not_null()
                            .default("complete"),
                    )
                    .add_column(
                        ColumnDef::new(ActivityImports::ProcessingError)
                            .text()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(ActivityImports::ProcessingAttempts)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .add_column(
                        ColumnDef::new(ActivityImports::ProcessedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(ActivityImports::LastProcessingEventAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-activity-imports-user-status-stage-created-at")
                    .table(ActivityImports::Table)
                    .col(ActivityImports::UserId)
                    .col(ActivityImports::Status)
                    .col(ActivityImports::ProcessingStage)
                    .col(ActivityImports::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-activity-imports-activity-id")
                    .table(ActivityImports::Table)
                    .col(ActivityImports::ActivityId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx-activity-imports-activity-id")
                    .table(ActivityImports::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-activity-imports-user-status-stage-created-at")
                    .table(ActivityImports::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ActivityImports::Table)
                    .drop_column(ActivityImports::LastProcessingEventAt)
                    .drop_column(ActivityImports::ProcessedAt)
                    .drop_column(ActivityImports::ProcessingAttempts)
                    .drop_column(ActivityImports::ProcessingError)
                    .drop_column(ActivityImports::ProcessingStage)
                    .drop_column(ActivityImports::ActivityId)
                    .to_owned(),
            )
            .await
    }
}
