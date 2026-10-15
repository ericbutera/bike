use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE background_tasks
                 ALTER COLUMN status SET DEFAULT 'pending',
                 ALTER COLUMN created_at SET DEFAULT CURRENT_TIMESTAMP,
                 ALTER COLUMN updated_at SET DEFAULT CURRENT_TIMESTAMP;",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE background_tasks
                 ALTER COLUMN status DROP DEFAULT,
                 ALTER COLUMN created_at DROP DEFAULT,
                 ALTER COLUMN updated_at DROP DEFAULT;",
            )
            .await?;
        Ok(())
    }
}
