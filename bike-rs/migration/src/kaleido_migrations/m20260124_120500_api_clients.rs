use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum ApiClients {
    Table,
    Id,
    ClientId,
    ClientSecretHash,
    Name,
    Description,
    Scopes,
    OwnerUserId,
    CreatedAt,
    LastUsedAt,
    RevokedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ApiClients::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ApiClients::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ApiClients::ClientId)
                            .uuid()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(ApiClients::ClientSecretHash)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ApiClients::Name).string().not_null())
                    .col(ColumnDef::new(ApiClients::Description).text().null())
                    .col(ColumnDef::new(ApiClients::Scopes).json_binary().not_null())
                    .col(ColumnDef::new(ApiClients::OwnerUserId).integer().null())
                    .col(
                        ColumnDef::new(ApiClients::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(ApiClients::LastUsedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ApiClients::RevokedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_api_clients_owner_user")
                            .from(ApiClients::Table, ApiClients::OwnerUserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApiClients::Table).to_owned())
            .await
    }
}
