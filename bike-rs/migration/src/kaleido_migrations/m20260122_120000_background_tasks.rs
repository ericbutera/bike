use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BackgroundTasks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BackgroundTasks::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::TaskType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::Payload)
                            .json_binary()
                            .not_null(),
                    )
                    .col(ColumnDef::new(BackgroundTasks::Status).string().not_null())
                    .col(
                        ColumnDef::new(BackgroundTasks::Attempts)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::MaxAttempts)
                            .integer()
                            .not_null()
                            .default(3),
                    )
                    .col(ColumnDef::new(BackgroundTasks::Error).text().null())
                    .col(
                        ColumnDef::new(BackgroundTasks::ScheduledFor)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::StartedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::CompletedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for efficient worker polling
        manager
            .create_index(
                Index::create()
                    .name("idx_background_tasks_status_scheduled")
                    .table(BackgroundTasks::Table)
                    .col(BackgroundTasks::Status)
                    .col(BackgroundTasks::ScheduledFor)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BackgroundTasks::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum BackgroundTasks {
    Table,
    Id,
    TaskType,
    Payload,
    Status,
    Attempts,
    MaxAttempts,
    Error,
    ScheduledFor,
    CreatedAt,
    UpdatedAt,
    StartedAt,
    CompletedAt,
}
