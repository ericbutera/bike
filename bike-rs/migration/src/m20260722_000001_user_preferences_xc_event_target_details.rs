use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum UserPreferences {
    Table,
    XcGoalEventName,
    XcGoalTargetFinishTimeSeconds,
    XcGoalEventProfile,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(UserPreferences::Table)
                    .add_column(
                        ColumnDef::new(UserPreferences::XcGoalEventName)
                            .string()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(UserPreferences::XcGoalTargetFinishTimeSeconds)
                            .integer()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(UserPreferences::XcGoalEventProfile)
                            .string()
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
                    .drop_column(UserPreferences::XcGoalEventProfile)
                    .drop_column(UserPreferences::XcGoalTargetFinishTimeSeconds)
                    .drop_column(UserPreferences::XcGoalEventName)
                    .to_owned(),
            )
            .await
    }
}
