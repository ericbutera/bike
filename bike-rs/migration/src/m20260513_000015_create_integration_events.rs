use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum IntegrationEvents {
    Table,
    Id,
    UserId,
    Provider,
    EventType,
    Level,
    Message,
    ConnectionId,
    Payload,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(IntegrationEvents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(IntegrationEvents::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(IntegrationEvents::UserId).integer().null())
                    .col(
                        ColumnDef::new(IntegrationEvents::Provider)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IntegrationEvents::EventType)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(IntegrationEvents::Level).string().not_null())
                    .col(
                        ColumnDef::new(IntegrationEvents::Message)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IntegrationEvents::ConnectionId)
                            .integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(IntegrationEvents::Payload)
                            .json_binary()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(IntegrationEvents::CreatedAt)
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
                    .name("idx-integration-events-provider-created-at")
                    .table(IntegrationEvents::Table)
                    .col(IntegrationEvents::Provider)
                    .col(IntegrationEvents::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-integration-events-user-id-created-at")
                    .table(IntegrationEvents::Table)
                    .col(IntegrationEvents::UserId)
                    .col(IntegrationEvents::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-integration-events-connection-id-created-at")
                    .table(IntegrationEvents::Table)
                    .col(IntegrationEvents::ConnectionId)
                    .col(IntegrationEvents::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(IntegrationEvents::Table).to_owned())
            .await
    }
}
