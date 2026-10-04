use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(BackgroundTasks::Table)
                    .add_column_if_not_exists(ColumnDef::new(BackgroundTasks::Result).text().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(BackgroundTasks::Table)
                    .drop_column(BackgroundTasks::Result)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum BackgroundTasks {
    Table,
    Result,
}
