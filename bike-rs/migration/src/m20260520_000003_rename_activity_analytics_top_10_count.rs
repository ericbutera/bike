use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum ActivityAnalytics {
    Table,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ActivityAnalytics::Table)
                    .rename_column(Alias::new("top10_count"), Alias::new("top_10_count"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ActivityAnalytics::Table)
                    .rename_column(Alias::new("top_10_count"), Alias::new("top10_count"))
                    .to_owned(),
            )
            .await
    }
}
