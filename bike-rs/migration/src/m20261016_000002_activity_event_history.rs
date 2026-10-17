use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("activity_event_history"))
                    .col(ColumnDef::new(Alias::new("id")).text().not_null().primary_key())
                    .col(ColumnDef::new(Alias::new("activity_id")).integer())
                    .col(ColumnDef::new(Alias::new("user_id")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("source")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("occurred_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("recorded_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("before_json")).json_binary())
                    .col(ColumnDef::new(Alias::new("after_json")).json_binary())
                    .col(ColumnDef::new(Alias::new("context_json")).json_binary().not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("activity_event_history_owner_activity")
                    .table(Alias::new("activity_event_history"))
                    .col(Alias::new("user_id"))
                    .col(Alias::new("activity_id"))
                    .col(Alias::new("recorded_at"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("activity_event_history")).to_owned())
            .await
    }
}
