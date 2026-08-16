use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum ActivityImports {
    Table,
    ImportVersion,
}

const LEGACY_IMPORT_VERSION: i32 = 1;
const CURRENT_IMPORT_VERSION: i32 = 2;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ActivityImports::Table)
                    .add_column(
                        ColumnDef::new(ActivityImports::ImportVersion)
                            .integer()
                            .not_null()
                            .default(CURRENT_IMPORT_VERSION),
                    )
                    .to_owned(),
            )
            .await?;

        let update_existing = Query::update()
            .table(ActivityImports::Table)
            .value(ActivityImports::ImportVersion, LEGACY_IMPORT_VERSION)
            .to_owned();
        manager.get_connection().execute(&update_existing).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ActivityImports::Table)
                    .drop_column(ActivityImports::ImportVersion)
                    .to_owned(),
            )
            .await
    }
}
