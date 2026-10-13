use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        // Forward-only retirement. Activities, users and OAuth connections survive.
        db.execute_unprepared("DELETE FROM background_tasks WHERE task_type LIKE 'strava_%';
            DELETE FROM background_tasks t USING activity_imports i
                WHERE i.source='strava_sync' AND t.task_type IN ('process_activity_import','reprocess_activity_import')
                AND COALESCE(t.payload->'data'->>'import_id',t.payload->>'import_id')=i.id::text;
            DELETE FROM background_tasks t USING activities a
                WHERE a.source='strava_sync' AND t.task_type='reprocess_activity_import'
                AND COALESCE(t.payload->'data'->>'activity_id',t.payload->>'activity_id')=a.id::text;
            UPDATE activity_imports SET status='failed',processing_error='Old Strava jobs retired; request sync or backfill',processing_stage='retired'
                WHERE source='strava_sync' AND status IN ('queued','processing');
            DELETE FROM worker_processor_registry WHERE task_type LIKE 'strava_%';
            DELETE FROM processing_pipeline_runs WHERE entrypoint LIKE 'strava%';
            DROP TABLE IF EXISTS strava_delivery_intents;
            DROP TABLE IF EXISTS strava_workflows;
            DROP TABLE IF EXISTS strava_gateway_revocations;
            DROP TABLE IF EXISTS strava_gateway_watermarks;
            DROP TABLE IF EXISTS strava_gateway_receipts;
            DROP TABLE IF EXISTS strava_gateway_bindings;
            ALTER TABLE processing_pipeline_runs DROP COLUMN IF EXISTS gateway_history;
            DELETE FROM integration_events WHERE provider='strava' AND (event_type LIKE 'gateway.%' OR event_type LIKE 'workflow.%');").await?;
        Ok(())
    }
    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom("Strava retirement is forward-only".into()))
    }
}
