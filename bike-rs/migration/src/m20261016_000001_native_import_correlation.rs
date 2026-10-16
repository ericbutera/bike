use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("activity_import_attempts"))
                    .add_column(ColumnDef::new(Alias::new("command_id")).text())
                    .add_column(ColumnDef::new(Alias::new("temporal_context")).json_binary())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("activity_import_attempt_command")
                    .table(Alias::new("activity_import_attempts"))
                    .col(Alias::new("user_id"))
                    .col(Alias::new("command_id"))
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("strava_activity_states"))
                    .add_column(ColumnDef::new(Alias::new("bike_activity_id")).integer())
                    .add_column(ColumnDef::new(Alias::new("deleted_at")).timestamp_with_time_zone())
                    .add_column(
                        ColumnDef::new(Alias::new("updated_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE strava_activity_states SET updated_at=applied_at,
             deleted_at=CASE WHEN deleted THEN to_timestamp(event_time) ELSE NULL END",
            )
            .await?;
        // Preserve known live mappings. Deleted IDs remain in existing event mutations.
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE strava_activity_states AS state SET bike_activity_id=activity.id
             FROM activities AS activity
             JOIN strava_connections AS connection ON connection.user_id=activity.user_id
             WHERE activity.source='strava_sync'
               AND activity.source_correlation_id=state.activity_id::text
               AND connection.athlete_id=state.athlete_id
               AND connection.created_at=state.connection_generation
               AND NOT state.deleted",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("strava_activity_states"))
                    .drop_column(Alias::new("bike_activity_id"))
                    .drop_column(Alias::new("deleted_at"))
                    .drop_column(Alias::new("updated_at"))
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("activity_import_attempt_command")
                    .table(Alias::new("activity_import_attempts"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("activity_import_attempts"))
                    .drop_column(Alias::new("command_id"))
                    .drop_column(Alias::new("temporal_context"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
