use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            r#"
            CREATE TABLE activity_recordings (
                activity_id INTEGER PRIMARY KEY REFERENCES activities(id) ON DELETE CASCADE,
                format_version INTEGER NOT NULL CHECK (format_version > 0),
                object_path TEXT NOT NULL CHECK (object_path LIKE 'bike/tasks/recordings/%' OR object_path LIKE 'strava/recordings/%'),
                object_sha256 TEXT NOT NULL CHECK (object_sha256 ~ '^[0-9a-f]{64}$'),
                object_bytes BIGINT NOT NULL CHECK (object_bytes >= 0)
            );
            "#,
        ).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE activity_recordings;")
            .await?;
        Ok(())
    }
}
