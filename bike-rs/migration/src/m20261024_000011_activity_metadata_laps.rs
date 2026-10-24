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
            CREATE TABLE activity_metadata (
                activity_id INTEGER PRIMARY KEY REFERENCES activities(id) ON DELETE CASCADE,
                description TEXT, location TEXT, private_notes TEXT, gear_id TEXT, gear_name TEXT
            );
            CREATE TABLE activity_laps (
                activity_id INTEGER NOT NULL REFERENCES activities(id) ON DELETE CASCADE,
                position INTEGER NOT NULL CHECK (position >= 0),
                lap_index INTEGER NOT NULL, title TEXT NOT NULL,
                start_offset_seconds INTEGER,
                duration_seconds INTEGER,
                distance_meters DOUBLE PRECISION,
                elevation_gain_meters DOUBLE PRECISION,
                elevation_loss_meters DOUBLE PRECISION,
                average_speed_mps DOUBLE PRECISION,
                max_speed_mps DOUBLE PRECISION,
                average_heart_rate_bpm INTEGER,
                max_heart_rate_bpm INTEGER,
                average_cadence_rpm INTEGER,
                max_cadence_rpm INTEGER,
                calories INTEGER,
                PRIMARY KEY (activity_id, position)
            );
            "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE activity_laps; DROP TABLE activity_metadata;")
            .await?;
        Ok(())
    }
}
