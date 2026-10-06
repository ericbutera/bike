use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("heatmap_projection_recording_policy.sql"))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            "ALTER TABLE heatmap_projections DROP CONSTRAINT heatmap_current_recording_policy;
             DELETE FROM heatmap_chunks;
             UPDATE heatmap_user_states SET revision=revision+1;
             UPDATE heatmap_projections SET generation=generation+1, projection_version=3, status='pending', queued_at=NULL, error=NULL, min_x=NULL, min_y=NULL, max_x=NULL, max_y=NULL;
             ALTER TABLE heatmap_projections ALTER COLUMN projection_version SET DEFAULT 3;").await?;
        Ok(())
    }
}
