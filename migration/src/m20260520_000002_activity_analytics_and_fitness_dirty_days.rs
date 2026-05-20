use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum ActivityAnalytics {
    Table,
    ActivityId,
    UserId,
    SegmentEffortCount,
    AchievementCount,
    KomCount,
    Top10Count,
    PrCount,
    AchievementHighlightsJson,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum AnalyticsUserStates {
    Table,
    FitnessDirtyFromDay,
    LastFitnessRebuildAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ActivityAnalytics::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ActivityAnalytics::ActivityId)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ActivityAnalytics::UserId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityAnalytics::SegmentEffortCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ActivityAnalytics::AchievementCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ActivityAnalytics::KomCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ActivityAnalytics::Top10Count)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ActivityAnalytics::PrCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ActivityAnalytics::AchievementHighlightsJson)
                            .json()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ActivityAnalytics::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(ActivityAnalytics::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-activity-analytics-user-id")
                    .table(ActivityAnalytics::Table)
                    .col(ActivityAnalytics::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(AnalyticsUserStates::Table)
                    .add_column(
                        ColumnDef::new(AnalyticsUserStates::FitnessDirtyFromDay)
                            .date()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(AnalyticsUserStates::LastFitnessRebuildAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AnalyticsUserStates::Table)
                    .drop_column(AnalyticsUserStates::LastFitnessRebuildAt)
                    .drop_column(AnalyticsUserStates::FitnessDirtyFromDay)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-activity-analytics-user-id")
                    .table(ActivityAnalytics::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(ActivityAnalytics::Table).to_owned())
            .await?;

        Ok(())
    }
}