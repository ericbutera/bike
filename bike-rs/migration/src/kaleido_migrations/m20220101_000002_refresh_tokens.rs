use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum RefreshTokens {
    Table,
    Token,
    UserPid,
    ExpiresAt,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RefreshTokens::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RefreshTokens::Token)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(RefreshTokens::UserPid).uuid().not_null())
                    .col(
                        ColumnDef::new(RefreshTokens::ExpiresAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .to_owned(),
            )
            .await?;

        // index for lookups by user_id
        manager
            .create_index(
                Index::create()
                    .name("idx_refresh_tokens_user_pid")
                    .table(RefreshTokens::Table)
                    .col(RefreshTokens::UserPid)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_refresh_tokens_user_pid")
                    .table(RefreshTokens::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(RefreshTokens::Table).to_owned())
            .await?;

        Ok(())
    }
}
