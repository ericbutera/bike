use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum ActivityImportLocks {
    Table,
    Id,
    UserId,
    Source,
    Stage,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ActivityImportLocks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ActivityImportLocks::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ActivityImportLocks::UserId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityImportLocks::Source)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityImportLocks::Stage)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityImportLocks::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(ActivityImportLocks::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-activity-import-locks-user-id")
                    .table(ActivityImportLocks::Table)
                    .col(ActivityImportLocks::UserId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ActivityImportLocks::Table).to_owned())
            .await
    }
}
