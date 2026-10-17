use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Only committed, non-superseded provider mutations establish a provider
        // watermark. Bike's sync/backfill clock cannot be recovered as one.
        manager
            .get_connection()
            .execute_unprepared(
                "WITH source_times AS (
                SELECT s.athlete_id, s.activity_id,
                       COALESCE(MAX((e.event #>> '{webhook,event_time}')::bigint), 0) AS provider_time
                FROM strava_activity_states s
                LEFT JOIN strava_events e ON e.athlete_id = s.athlete_id AND e.activity_id = s.activity_id
                    AND (e.event ->> 'connection_generation')::timestamptz = s.connection_generation
                    AND e.mutation IS NOT NULL
                    AND e.status IN ('applied', 'unchanged', 'deleted', 'already_absent')
                GROUP BY s.athlete_id, s.activity_id
             )
             UPDATE strava_activity_states s SET
                event_time = t.provider_time,
                deleted_at = CASE WHEN s.deleted AND s.deleted_at = to_timestamp(t.provider_time)
                                  THEN s.deleted_at ELSE NULL END,
                updated_at = GREATEST(s.updated_at + INTERVAL '1 microsecond', CURRENT_TIMESTAMP),
                applied_at = GREATEST(s.updated_at + INTERVAL '1 microsecond', CURRENT_TIMESTAMP)
             FROM source_times t WHERE s.athlete_id = t.athlete_id AND s.activity_id = t.activity_id",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Data repair is intentionally irreversible: do not restore mixed clocks.
        Ok(())
    }
}
