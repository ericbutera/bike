use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum Segments {
    Table,
    Mode,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Segments::Table)
                    .add_column(
                        ColumnDef::new(Segments::Mode)
                            .string_len(16)
                            .not_null()
                            .default("xc"),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Segments::Table)
                    .drop_column(Segments::Mode)
                    .to_owned(),
            )
            .await
    }
}
