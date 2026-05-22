use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum ActivityTrainingAnalyses {
    Table,
    RideFocus,
    RouteFamilyKey,
    ComparableDistanceBucketMeters,
    ComparableElevationGainBucketMeters,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ActivityTrainingAnalyses::Table)
                    .add_column(
                        ColumnDef::new(ActivityTrainingAnalyses::RideFocus)
                            .string_len(32)
                            .not_null()
                            .default("other"),
                    )
                    .add_column(
                        ColumnDef::new(ActivityTrainingAnalyses::RouteFamilyKey)
                            .string_len(128)
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(ActivityTrainingAnalyses::ComparableDistanceBucketMeters)
                            .integer()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(
                            ActivityTrainingAnalyses::ComparableElevationGainBucketMeters,
                        )
                        .integer()
                        .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ActivityTrainingAnalyses::Table)
                    .drop_column(ActivityTrainingAnalyses::ComparableElevationGainBucketMeters)
                    .drop_column(ActivityTrainingAnalyses::ComparableDistanceBucketMeters)
                    .drop_column(ActivityTrainingAnalyses::RouteFamilyKey)
                    .drop_column(ActivityTrainingAnalyses::RideFocus)
                    .to_owned(),
            )
            .await
    }
}