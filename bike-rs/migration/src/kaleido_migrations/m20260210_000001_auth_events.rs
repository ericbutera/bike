use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuthEvents::Table)
                    .if_not_exists()
                    .col(pk_auto(AuthEvents::Id))
                    .col(
                        ColumnDef::new(AuthEvents::Ts)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(ColumnDef::new(AuthEvents::EventType).string().not_null())
                    .col(ColumnDef::new(AuthEvents::UserId).integer().null())
                    .col(ColumnDef::new(AuthEvents::Email).string().null())
                    .col(ColumnDef::new(AuthEvents::Ip).string().null())
                    .col(ColumnDef::new(AuthEvents::UserAgent).text().null())
                    .col(ColumnDef::new(AuthEvents::Reason).text().null())
                    .col(ColumnDef::new(AuthEvents::Meta).json_binary().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_auth_events_ts")
                    .table(AuthEvents::Table)
                    .col(AuthEvents::Ts)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_auth_events_event_type")
                    .table(AuthEvents::Table)
                    .col(AuthEvents::EventType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_auth_events_user_id")
                    .table(AuthEvents::Table)
                    .col(AuthEvents::UserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuthEvents::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AuthEvents {
    Table,
    Id,
    Ts,
    EventType,
    UserId,
    Email,
    Ip,
    UserAgent,
    Reason,
    Meta,
}
