use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE synthetic_scenarios (
                name varchar PRIMARY KEY,
                user_id integer NOT NULL UNIQUE REFERENCES users(id),
                activity_id integer NOT NULL REFERENCES activities(id),
                segment_id integer NOT NULL REFERENCES segments(id),
                first_effort_id integer NOT NULL REFERENCES segment_efforts(id),
                second_effort_id integer NOT NULL REFERENCES segment_efforts(id)
            );",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE synthetic_scenarios;")
            .await?;
        Ok(())
    }
}
