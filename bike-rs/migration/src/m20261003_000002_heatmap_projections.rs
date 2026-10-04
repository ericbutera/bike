use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("heatmap_projections.sql"))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            "DROP TRIGGER heatmap_activity_changed ON activities; DROP FUNCTION heatmap_activity_changed(); DROP TABLE heatmap_chunks; DROP TABLE heatmap_projections; DROP TABLE heatmap_user_states;",
        ).await?;
        Ok(())
    }
}
