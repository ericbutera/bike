use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum ActivityTrainingAnalyses {
    Table,
    AerobicDecouplingPercent,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ActivityTrainingAnalyses::Table)
                    .add_column(
                        ColumnDef::new(ActivityTrainingAnalyses::AerobicDecouplingPercent)
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
                    .table(ActivityTrainingAnalyses::Table)
                    .drop_column(ActivityTrainingAnalyses::AerobicDecouplingPercent)
                    .to_owned(),
            )
            .await
    }
}