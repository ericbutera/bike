use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            "ALTER TABLE strava_gateway_bindings ADD COLUMN active_delivery_id text, ADD COLUMN lease_until timestamptz",
        ).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            "ALTER TABLE strava_gateway_bindings DROP COLUMN active_delivery_id, DROP COLUMN lease_until",
        ).await?;
        Ok(())
    }
}
