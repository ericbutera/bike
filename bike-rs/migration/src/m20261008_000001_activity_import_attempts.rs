use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("activity_imports"))
                    .add_column(ColumnDef::new(Alias::new("archive_job_id")).integer())
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Attempts::Table)
                    .col(
                        ColumnDef::new(Attempts::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Attempts::UserId).integer().not_null())
                    .col(
                        ColumnDef::new(Attempts::ActivityImportId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Attempts::ActivityId).integer())
                    .col(ColumnDef::new(Attempts::Status).string().not_null())
                    .col(ColumnDef::new(Attempts::RequestedStage).string().not_null())
                    .col(ColumnDef::new(Attempts::StartStage).string().not_null())
                    .col(ColumnDef::new(Attempts::CurrentStage).string().not_null())
                    .col(ColumnDef::new(Attempts::ReusedAttemptId).integer())
                    .col(
                        ColumnDef::new(Attempts::SourceJson)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Attempts::StagesJson)
                            .json_binary()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Attempts::CheckpointJson).json_binary())
                    .col(ColumnDef::new(Attempts::Error).text())
                    .col(
                        ColumnDef::new(Attempts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Attempts::StartedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Attempts::FinishedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(Attempts::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Attempts::Table, Attempts::ActivityImportId)
                            .to(Alias::new("activity_imports"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("activity_import_attempt_history")
                    .table(Attempts::Table)
                    .col(Attempts::UserId)
                    .col(Attempts::ActivityImportId)
                    .col(Attempts::Id)
                    .to_owned(),
            )
            .await?;
        // Partial unique indexes enforce one live execution across workers and API requests.
        manager.get_connection().execute_unprepared(
            "CREATE UNIQUE INDEX activity_import_attempt_active_import ON activity_import_attempts (activity_import_id) WHERE status IN ('queued', 'running')",
        ).await?;
        manager.get_connection().execute_unprepared(
            "CREATE UNIQUE INDEX activity_import_attempt_active_activity ON activity_import_attempts (activity_id) WHERE status IN ('queued', 'running') AND activity_id IS NOT NULL",
        ).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Attempts::Table).to_owned())
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("activity_imports"))
                    .drop_column(Alias::new("archive_job_id"))
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Attempts {
    #[sea_orm(iden = "activity_import_attempts")]
    Table,
    Id,
    UserId,
    ActivityImportId,
    ActivityId,
    Status,
    RequestedStage,
    StartStage,
    CurrentStage,
    ReusedAttemptId,
    SourceJson,
    StagesJson,
    CheckpointJson,
    Error,
    CreatedAt,
    StartedAt,
    FinishedAt,
    UpdatedAt,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection};

    #[tokio::test]
    #[ignore = "Set BIKE_INGESTION_TEST_DATABASE_URL to a disposable PostgreSQL database"]
    async fn postgres_migration_enforces_active_execution_uniqueness_and_preserves_history() {
        let url =
            std::env::var("BIKE_INGESTION_TEST_DATABASE_URL").expect("disposable database URL");
        let db = Database::connect(&url).await.unwrap();
        let schema = format!("ingestion_{}", std::process::id());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}"))
            .await
            .unwrap();
        let mut options = sea_orm_migration::sea_orm::ConnectOptions::new(url);
        options.set_schema_search_path(schema.clone());
        let isolated = Database::connect(options).await.unwrap();
        isolated.execute_unprepared("CREATE TABLE activity_imports(id integer PRIMARY KEY); INSERT INTO activity_imports VALUES (1), (2)").await.unwrap();
        let manager = SchemaManager::new(&isolated);
        Migration.up(&manager).await.unwrap();
        insert_attempt(&isolated, 1, 42).await.unwrap();
        assert!(insert_attempt(&isolated, 1, 43).await.is_err());
        assert!(insert_attempt(&isolated, 2, 42).await.is_err());
        isolated
            .execute_unprepared(
                "UPDATE activity_import_attempts SET status='completed' WHERE activity_import_id=1",
            )
            .await
            .unwrap();
        insert_attempt(&isolated, 1, 42).await.unwrap();
        isolated
            .execute_unprepared("DELETE FROM activity_imports WHERE id=1")
            .await
            .unwrap();
        let count = isolated
            .query_one_raw(sea_orm_migration::sea_orm::Statement::from_string(
                sea_orm_migration::sea_orm::DbBackend::Postgres,
                "SELECT count(*) AS total FROM activity_import_attempts",
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get::<i64>("", "total")
            .unwrap();
        assert_eq!(count, 0);
        Migration.down(&manager).await.unwrap();
        assert!(!manager.has_table("activity_import_attempts").await.unwrap());
        assert!(!manager
            .has_column("activity_imports", "archive_job_id")
            .await
            .unwrap());
        isolated
            .execute_unprepared("DROP TABLE activity_imports")
            .await
            .unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema}"))
            .await
            .unwrap();
    }

    async fn insert_attempt(
        db: &DatabaseConnection,
        import_id: i32,
        activity_id: i32,
    ) -> Result<(), DbErr> {
        db.execute_unprepared(&format!("INSERT INTO activity_import_attempts(user_id,activity_import_id,activity_id,status,requested_stage,start_stage,current_stage,source_json,stages_json,created_at,updated_at) VALUES (1,{import_id},{activity_id},'queued','raw_stored','raw_stored','raw_stored','{{}}','[]',now(),now())")).await?;
        Ok(())
    }
}
