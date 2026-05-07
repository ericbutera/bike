use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum ActivityImports {
    Table,
    Id,
    UserId,
    Source,
    Format,
    Status,
    OriginalFilename,
    StoragePath,
    SizeBytes,
    MimeType,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ActivityImports::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ActivityImports::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ActivityImports::UserId).integer().not_null())
                    .col(ColumnDef::new(ActivityImports::Source).string().not_null())
                    .col(ColumnDef::new(ActivityImports::Format).string().not_null())
                    .col(ColumnDef::new(ActivityImports::Status).string().not_null())
                    .col(
                        ColumnDef::new(ActivityImports::OriginalFilename)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityImports::StoragePath)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActivityImports::SizeBytes)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ActivityImports::MimeType).string().null())
                    .col(
                        ColumnDef::new(ActivityImports::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(ActivityImports::UpdatedAt)
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
                    .name("idx-activity-imports-user-id")
                    .table(ActivityImports::Table)
                    .col(ActivityImports::UserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ActivityImports::Table).to_owned())
            .await
    }
}