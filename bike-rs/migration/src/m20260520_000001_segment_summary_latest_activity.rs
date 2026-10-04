use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum SegmentSummaries {
    Table,
    LatestActivityStartedAt,
    LatestActivityId,
    LatestEffortId,
}

#[derive(Iden)]
enum SegmentEfforts {
    Table,
    SegmentId,
    DurationSeconds,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SegmentSummaries::Table)
                    .add_column(
                        ColumnDef::new(SegmentSummaries::LatestActivityStartedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(SegmentSummaries::LatestActivityId)
                            .integer()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(SegmentSummaries::LatestEffortId)
                            .integer()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-segment-efforts-segment-duration-id")
                    .table(SegmentEfforts::Table)
                    .col(SegmentEfforts::SegmentId)
                    .col(SegmentEfforts::DurationSeconds)
                    .col(SegmentEfforts::Id)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx-segment-efforts-segment-duration-id")
                    .table(SegmentEfforts::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SegmentSummaries::Table)
                    .drop_column(SegmentSummaries::LatestEffortId)
                    .drop_column(SegmentSummaries::LatestActivityId)
                    .drop_column(SegmentSummaries::LatestActivityStartedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
