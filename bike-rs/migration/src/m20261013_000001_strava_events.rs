use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("strava_connections"))
                    .add_column(ColumnDef::new(Alias::new("disconnect_requested_at")).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("strava_events"))
                    .col(ColumnDef::new(Alias::new("id")).string().primary_key())
                    .col(ColumnDef::new(Alias::new("event_key")).string().not_null().unique_key())
                    .col(ColumnDef::new(Alias::new("correlation_id")).string().not_null())
                    .col(ColumnDef::new(Alias::new("user_id")).integer().not_null())
                    .col(ColumnDef::new(Alias::new("parent_event_id")).string())
                    .col(ColumnDef::new(Alias::new("customer_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("athlete_id")).big_integer().not_null())
                    .col(ColumnDef::new(Alias::new("activity_id")).big_integer())
                    .col(
                        ColumnDef::new(Alias::new("workflow_id"))
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Alias::new("event")).json_binary().not_null())
                    .col(ColumnDef::new(Alias::new("mutation")).json_binary())
                    .col(
                        ColumnDef::new(Alias::new("accepted_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("submitted_at")).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(Alias::new("next_attempt_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("handoff_error")).text())
                    .to_owned(),
            )
            .await?;
        for column in [
            "parent_event_id",
            "user_id",
            "customer_id",
            "athlete_id",
            "activity_id",
            "correlation_id",
            "submitted_at",
            "next_attempt_at",
        ] {
            manager
                .create_index(
                    Index::create()
                        .name(format!("strava_events_{column}_idx"))
                        .table(Alias::new("strava_events"))
                        .col(Alias::new(column))
                        .to_owned(),
                )
                .await?;
        }
        manager.get_connection().execute_unprepared(
            "CREATE INDEX integration_events_strava_event_idx ON integration_events ((payload->>'strava_event_id')) WHERE provider='strava'"
        ).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX integration_events_strava_event_idx")
            .await?;
        manager
            .drop_table(Table::drop().table(Alias::new("strava_events")).to_owned())
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("strava_connections"))
                    .drop_column(Alias::new("disconnect_requested_at"))
                    .to_owned(),
            )
            .await
    }
}
