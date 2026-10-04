use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum Segments {
    Table,
    SourceActivityId,
    SourceStartRoutePointIndex,
    SourceEndRoutePointIndex,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Segments::Table)
                    .add_column(ColumnDef::new(Segments::SourceActivityId).integer().null())
                    .add_column(
                        ColumnDef::new(Segments::SourceStartRoutePointIndex)
                            .integer()
                            .null(),
                    )
                    .add_column(
                        ColumnDef::new(Segments::SourceEndRoutePointIndex)
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
                    .table(Segments::Table)
                    .drop_column(Segments::SourceEndRoutePointIndex)
                    .drop_column(Segments::SourceStartRoutePointIndex)
                    .drop_column(Segments::SourceActivityId)
                    .to_owned(),
            )
            .await
    }
}
