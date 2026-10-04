use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
            CREATE TABLE strava_gateway_bindings (
                athlete_id bigint PRIMARY KEY,
                user_id integer NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
                created_at timestamptz NOT NULL DEFAULT now()
            );
            CREATE TABLE strava_gateway_receipts (
                delivery_id text PRIMARY KEY,
                athlete_id bigint NOT NULL REFERENCES strava_gateway_bindings(athlete_id),
                activity_id bigint NOT NULL,
                event_time bigint NOT NULL,
                operation text NOT NULL CHECK (operation IN ('upsert','delete','deauthorize')),
                status text NOT NULL CHECK (status IN ('processing','completed')),
                lease_until timestamptz,
                created_at timestamptz NOT NULL DEFAULT now(),
                completed_at timestamptz
            );
            CREATE INDEX strava_gateway_receipts_processing_idx
                ON strava_gateway_receipts (lease_until) WHERE status='processing';
            CREATE TABLE strava_gateway_watermarks (
                athlete_id bigint NOT NULL REFERENCES strava_gateway_bindings(athlete_id),
                activity_id bigint NOT NULL,
                event_time bigint NOT NULL,
                operation text NOT NULL,
                PRIMARY KEY (athlete_id, activity_id)
            );
            "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            "DROP TABLE strava_gateway_watermarks; DROP TABLE strava_gateway_receipts; DROP TABLE strava_gateway_bindings;",
        ).await?;
        Ok(())
    }
}
