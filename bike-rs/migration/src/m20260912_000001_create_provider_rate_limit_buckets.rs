use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum ProviderRateLimitBuckets {
    Table,
    Id,
    Provider,
    Bucket,
    LimitCount,
    UsedCount,
    ResetAt,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProviderRateLimitBuckets::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProviderRateLimitBuckets::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProviderRateLimitBuckets::Provider)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProviderRateLimitBuckets::Bucket)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProviderRateLimitBuckets::LimitCount)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProviderRateLimitBuckets::UsedCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ProviderRateLimitBuckets::ResetAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProviderRateLimitBuckets::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(ProviderRateLimitBuckets::UpdatedAt)
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
                    .name("idx-provider-rate-limit-buckets-provider-bucket")
                    .table(ProviderRateLimitBuckets::Table)
                    .col(ProviderRateLimitBuckets::Provider)
                    .col(ProviderRateLimitBuckets::Bucket)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ProviderRateLimitBuckets::Table)
                    .to_owned(),
            )
            .await
    }
}
