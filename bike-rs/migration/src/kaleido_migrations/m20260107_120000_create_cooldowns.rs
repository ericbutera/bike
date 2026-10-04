use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum Cooldowns {
    Table,
    Id,
    SubjectType,
    SubjectId,
    Action,
    LastRun,
    AttemptCount,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Cooldowns::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Cooldowns::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Cooldowns::SubjectType).string().not_null())
                    .col(ColumnDef::new(Cooldowns::SubjectId).integer().null())
                    .col(ColumnDef::new(Cooldowns::Action).string().not_null())
                    .col(
                        ColumnDef::new(Cooldowns::LastRun)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Cooldowns::AttemptCount)
                            .integer()
                            .not_null()
                            .default(Expr::value(0)),
                    )
                    .col(
                        ColumnDef::new(Cooldowns::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(Cooldowns::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Cooldowns::Table).to_owned())
            .await?;
        Ok(())
    }
}
