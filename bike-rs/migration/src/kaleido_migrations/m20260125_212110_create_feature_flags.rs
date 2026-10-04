use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FeatureFlags::Table)
                    .if_not_exists()
                    .col(pk_auto(FeatureFlags::Id))
                    .col(string_uniq(FeatureFlags::FeatureKey))
                    .col(boolean(FeatureFlags::Enabled).default(false))
                    .col(string_null(FeatureFlags::Description))
                    .col(
                        ColumnDef::new(FeatureFlags::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(FeatureFlags::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .to_owned(),
            )
            .await?;

        let insert = Query::insert()
            .into_table(FeatureFlags::Table)
            .columns([
                FeatureFlags::FeatureKey,
                FeatureFlags::Enabled,
                FeatureFlags::Description,
            ])
            .values_panic([
                "oauth".into(),
                true.into(),
                "Enable OAuth authentication providers".into(),
            ])
            .values_panic([
                "api_clients".into(),
                true.into(),
                "Enable API client authentication".into(),
            ])
            .values_panic([
                "registration".into(),
                true.into(),
                "Enable user registration".into(),
            ])
            .to_owned();

        manager.exec_stmt(insert).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FeatureFlags::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum FeatureFlags {
    Table,
    Id,
    FeatureKey,
    Enabled,
    Description,
    CreatedAt,
    UpdatedAt,
}
