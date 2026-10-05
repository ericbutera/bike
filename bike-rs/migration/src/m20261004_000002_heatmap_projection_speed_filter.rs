use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("heatmap_projection_speed_filter.sql"))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE heatmap_user_states SET revision = revision + 1 WHERE user_id IN (SELECT DISTINCT user_id FROM heatmap_projections WHERE projection_version > 1);
                 UPDATE heatmap_projections SET generation = generation + 1, projection_version = 1, status = 'pending', queued_at = NULL, error = NULL, min_x = NULL, min_y = NULL, max_x = NULL, max_y = NULL WHERE projection_version > 1;
                 ALTER TABLE heatmap_projections ALTER COLUMN projection_version SET DEFAULT 1;",
            )
            .await?;
        Ok(())
    }
}
