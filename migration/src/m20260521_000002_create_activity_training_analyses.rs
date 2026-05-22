use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum ActivityTrainingAnalyses {
    Table,
    ActivityId,
    UserId,
    Z2TimeSeconds,
    Z2DistanceMeters,
    Z2AverageSpeedMps,
    ClimbingTimeSeconds,
    ClimbingElevationGainMeters,
    SustainedClimbCount,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ActivityTrainingAnalyses::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ActivityTrainingAnalyses::ActivityId)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ActivityTrainingAnalyses::UserId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityTrainingAnalyses::Z2TimeSeconds)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ActivityTrainingAnalyses::Z2DistanceMeters)
                            .double()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ActivityTrainingAnalyses::Z2AverageSpeedMps)
                            .double()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ActivityTrainingAnalyses::ClimbingTimeSeconds)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ActivityTrainingAnalyses::ClimbingElevationGainMeters)
                            .double()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ActivityTrainingAnalyses::SustainedClimbCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ActivityTrainingAnalyses::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(ActivityTrainingAnalyses::UpdatedAt)
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
                    .name("idx-activity-training-analyses-user-id")
                    .table(ActivityTrainingAnalyses::Table)
                    .col(ActivityTrainingAnalyses::UserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx-activity-training-analyses-user-id")
                    .table(ActivityTrainingAnalyses::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(ActivityTrainingAnalyses::Table)
                    .to_owned(),
            )
            .await
    }
}
