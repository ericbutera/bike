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
                ALTER TABLE activity_climbs
                    ADD COLUMN vertical_rate_meters_per_hour DOUBLE PRECISION,
                    ADD COLUMN average_speed_mps DOUBLE PRECISION,
                    ADD COLUMN average_heart_rate_bpm DOUBLE PRECISION,
                    ADD COLUMN peak_heart_rate_bpm INTEGER,
                    ADD COLUMN average_cadence_rpm DOUBLE PRECISION,
                    ADD COLUMN average_power_watts DOUBLE PRECISION,
                    ADD COLUMN heart_rate_recovery_30_seconds_bpm INTEGER,
                    ADD COLUMN heart_rate_recovery_60_seconds_bpm INTEGER,
                    ADD COLUMN seconds_to_drop_10_bpm INTEGER,
                    ADD COLUMN seconds_to_drop_15_bpm INTEGER,
                    ADD COLUMN summit_immediately_enters_descent BOOLEAN,
                    ADD COLUMN is_first_half BOOLEAN;
                ALTER TABLE activity_climb_outputs
                    ADD COLUMN object_path TEXT,
                    ADD COLUMN object_sha256 TEXT,
                    ADD COLUMN object_bytes BIGINT;
                UPDATE activity_climb_outputs SET
                    object_path = object_ref->>'path',
                    object_sha256 = object_ref->>'sha256',
                    object_bytes = (object_ref->>'bytes')::BIGINT;
                ALTER TABLE activity_climb_outputs
                    ALTER COLUMN object_path SET NOT NULL,
                    ALTER COLUMN object_sha256 SET NOT NULL,
                    ALTER COLUMN object_bytes SET NOT NULL,
                    ADD CONSTRAINT activity_climb_object_path CHECK (object_path LIKE 'activity-imports/%'),
                    ADD CONSTRAINT activity_climb_object_sha256 CHECK (object_sha256 ~ '^[0-9a-f]{64}$'),
                    ADD CONSTRAINT activity_climb_object_bytes CHECK (object_bytes >= 0),
                    DROP COLUMN object_ref;
                "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE activity_climb_outputs ADD COLUMN object_ref JSONB;
                UPDATE activity_climb_outputs SET object_ref = jsonb_build_object(
                    'path', object_path, 'sha256', object_sha256, 'bytes', object_bytes
                );
                ALTER TABLE activity_climb_outputs
                    ALTER COLUMN object_ref SET NOT NULL,
                    DROP COLUMN object_path,
                    DROP COLUMN object_sha256,
                    DROP COLUMN object_bytes;
                ALTER TABLE activity_climbs
                    DROP COLUMN vertical_rate_meters_per_hour,
                    DROP COLUMN average_speed_mps,
                    DROP COLUMN average_heart_rate_bpm,
                    DROP COLUMN peak_heart_rate_bpm,
                    DROP COLUMN average_cadence_rpm,
                    DROP COLUMN average_power_watts,
                    DROP COLUMN heart_rate_recovery_30_seconds_bpm,
                    DROP COLUMN heart_rate_recovery_60_seconds_bpm,
                    DROP COLUMN seconds_to_drop_10_bpm,
                    DROP COLUMN seconds_to_drop_15_bpm,
                    DROP COLUMN summit_immediately_enters_descent,
                    DROP COLUMN is_first_half;
                "#,
            )
            .await?;
        Ok(())
    }
}
