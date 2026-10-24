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
            CREATE TABLE activity_cards (
                activity_id INTEGER PRIMARY KEY REFERENCES activities(id) ON DELETE CASCADE,
                route_location TEXT,
                location TEXT
            );
            CREATE TABLE activity_card_preview_points (
                activity_id INTEGER NOT NULL REFERENCES activity_cards(activity_id) ON DELETE CASCADE,
                point_number INTEGER NOT NULL CHECK (point_number BETWEEN 1 AND 24),
                elapsed_seconds INTEGER NOT NULL,
                latitude DOUBLE PRECISION NOT NULL CHECK (latitude BETWEEN -90 AND 90),
                longitude DOUBLE PRECISION NOT NULL CHECK (longitude BETWEEN -180 AND 180),
                PRIMARY KEY (activity_id, point_number)
            );
        "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE activity_card_preview_points; DROP TABLE activity_cards;")
            .await?;
        Ok(())
    }
}
