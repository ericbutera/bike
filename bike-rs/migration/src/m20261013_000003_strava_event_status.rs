use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            "ALTER TABLE strava_events ADD COLUMN status varchar(32) NOT NULL DEFAULT 'pending';
             UPDATE strava_events
             SET status = COALESCE(mutation->>'outcome', mutation->>'status', 'pending')
             WHERE mutation IS NOT NULL;
             UPDATE strava_events
             SET mutation = mutation - 'outcome' - 'status'
             WHERE mutation IS NOT NULL;
             ALTER TABLE strava_events ADD CONSTRAINT strava_events_status_check
             CHECK (status IN ('pending', 'captured', 'applied', 'unchanged', 'deleted', 'already_absent', 'superseded'));
             CREATE INDEX strava_events_parent_status_idx ON strava_events (parent_event_id, user_id, status);"
        ).await?;
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom("Strava event status migration is forward-only".into()))
    }
}
