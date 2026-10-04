use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum UserPreferences {
    Table,
    XcGoalBackfillStatus,
    XcGoalBackfillCompletedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(UserPreferences::Table)
                    .add_column(
                        ColumnDef::new(UserPreferences::XcGoalBackfillStatus)
                            .string()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(UserPreferences::XcGoalBackfillCompletedAt)
                            .timestamp_with_time_zone()
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
                    .drop_column(UserPreferences::XcGoalBackfillCompletedAt)
                    .drop_column(UserPreferences::XcGoalBackfillStatus)
                    .to_owned(),
            )
            .await
    }
}
