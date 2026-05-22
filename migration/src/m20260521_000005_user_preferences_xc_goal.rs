use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum UserPreferences {
    Table,
    XcGoalTargetDate,
    XcGoalTargetDistanceMeters,
    XcGoalTargetElevationGainMeters,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(UserPreferences::Table)
                    .add_column(
                        ColumnDef::new(UserPreferences::XcGoalTargetDate)
                            .date()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(UserPreferences::XcGoalTargetDistanceMeters)
                            .double()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(UserPreferences::XcGoalTargetElevationGainMeters)
                            .double()
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
                    .table(UserPreferences::Table)
                    .drop_column(UserPreferences::XcGoalTargetElevationGainMeters)
                    .drop_column(UserPreferences::XcGoalTargetDistanceMeters)
                    .drop_column(UserPreferences::XcGoalTargetDate)
                    .to_owned(),
            )
            .await
    }
}