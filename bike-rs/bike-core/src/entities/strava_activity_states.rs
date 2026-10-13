use crate::strava::events::StravaEvent;
use sea_orm::entity::prelude::*;
use sea_orm::{sea_query::OnConflict, ConnectionTrait, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "strava_activity_states")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub athlete_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i64,
    pub connection_generation: DateTimeUtc,
    pub event_time: i64,
    pub applied_hash: Option<String>,
    pub deleted: bool,
    pub applied_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
impl Model {
    pub async fn matches_content(
        db: &impl ConnectionTrait,
        event: &StravaEvent,
        hash: &str,
    ) -> Result<bool, DbErr> {
        Self::matches_by_id(db, event, |state| state.matches_event_content(event, hash)).await
    }
    pub async fn supersedes(
        db: &impl ConnectionTrait,
        event: &StravaEvent,
        deletion: bool,
    ) -> Result<bool, DbErr> {
        Self::matches_by_id(db, event, |state| state.supersedes_event(event, deletion)).await
    }

    async fn matches_by_id(
        db: &impl ConnectionTrait,
        event: &StravaEvent,
        predicate: impl FnOnce(Self) -> bool,
    ) -> Result<bool, DbErr> {
        Ok(Entity::find_by_id(Self::key_for_event(event)?)
            .one(db)
            .await?
            .is_some_and(predicate))
    }

    fn key_for_event(event: &StravaEvent) -> Result<(i64, i64), DbErr> {
        let id = event
            .activity_id()
            .map_err(|error| DbErr::Custom(error.message))?;
        Ok((event.strava_athlete_id, id))
    }

    fn matches_event_content(&self, event: &StravaEvent, hash: &str) -> bool {
        let same_content = !self.deleted && self.applied_hash.as_deref() == Some(hash);
        self.has_same_generation(event) && same_content
    }

    fn supersedes_event(&self, event: &StravaEvent, deletion: bool) -> bool {
        self.has_same_generation(event) && self.is_newer_than(event, deletion)
    }

    fn has_same_generation(&self, event: &StravaEvent) -> bool {
        self.connection_generation == event.connection_generation
    }

    fn is_newer_than(&self, event: &StravaEvent, deletion: bool) -> bool {
        self.event_time > event.event_time || self.is_deletion_at(event.event_time, deletion)
    }

    fn is_deletion_at(&self, event_time: i64, deletion: bool) -> bool {
        let delete_first = self.deleted && !deletion;
        self.event_time == event_time && delete_first
    }

    pub async fn apply_event(
        db: &impl ConnectionTrait,
        event: &StravaEvent,
        hash: Option<String>,
        deleted: bool,
    ) -> Result<(), DbErr> {
        let (athlete_id, activity_id) = Self::key_for_event(event)?;
        Self {
            athlete_id,
            activity_id,
            connection_generation: event.connection_generation,
            event_time: event.event_time,
            applied_hash: hash,
            deleted,
            applied_at: chrono::Utc::now(),
        }
        .save(db)
        .await
    }
    pub async fn save(self, db: &impl ConnectionTrait) -> Result<(), DbErr> {
        Entity::insert(ActiveModel {
            athlete_id: Set(self.athlete_id),
            activity_id: Set(self.activity_id),
            connection_generation: Set(self.connection_generation),
            event_time: Set(self.event_time),
            applied_hash: Set(self.applied_hash),
            deleted: Set(self.deleted),
            applied_at: Set(self.applied_at),
        })
        .on_conflict(
            OnConflict::columns([Column::AthleteId, Column::ActivityId])
                .update_columns([
                    Column::ConnectionGeneration,
                    Column::EventTime,
                    Column::AppliedHash,
                    Column::Deleted,
                    Column::AppliedAt,
                ])
                .to_owned(),
        )
        .exec(db)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strava::native::strava_native_unit::fixture::Fixture;
    use sea_orm::{DatabaseBackend, MockDatabase};

    fn state(fixture: &Fixture) -> Model {
        Model {
            athlete_id: fixture.event.strava_athlete_id,
            activity_id: fixture.event.activity_id().unwrap(),
            connection_generation: fixture.event.connection_generation,
            event_time: fixture.event.event_time,
            applied_hash: Some("fixture-content".into()),
            deleted: false,
            applied_at: fixture.connection.created_at,
        }
    }

    #[tokio::test]
    async fn content_matches_only_live_state_from_the_same_connection() {
        let fixture = Fixture::new();
        let matching = state(&fixture);
        let mut deleted = matching.clone();
        deleted.deleted = true;
        let mut reconnected = matching.clone();
        reconnected.connection_generation += chrono::Duration::seconds(1);
        for (stored, hash, expected) in [
            (None, "fixture-content", false),
            (Some(matching.clone()), "fixture-content", true),
            (Some(matching), "changed-content", false),
            (Some(deleted), "fixture-content", false),
            (Some(reconnected), "fixture-content", false),
        ] {
            let db = MockDatabase::new(DatabaseBackend::Postgres)
                .append_query_results([stored.into_iter().collect::<Vec<_>>()])
                .into_connection();
            assert_eq!(
                Model::matches_content(&db, &fixture.event, hash)
                    .await
                    .unwrap(),
                expected
            );
            let transactions = db.into_transaction_log();
            let values = &transactions[0].statements()[0].values.as_ref().unwrap().0;
            assert!(values.contains(&fixture.event.strava_athlete_id.into()));
            assert!(values.contains(&fixture.event.activity_id().unwrap().into()));
        }
    }

    #[tokio::test]
    async fn supersession_keeps_connection_generation_and_delete_tie_precedence() {
        let fixture = Fixture::new();
        let matching = state(&fixture);
        let mut newer = matching.clone();
        newer.event_time += 1;
        let mut older = matching.clone();
        older.event_time -= 1;
        let mut deleted = matching.clone();
        deleted.deleted = true;
        let mut reconnected = newer.clone();
        reconnected.connection_generation += chrono::Duration::seconds(1);
        for (stored, deletion, expected) in [
            (None, false, false),
            (Some(matching), false, false),
            (Some(newer.clone()), false, true),
            (Some(newer), true, true),
            (Some(older), false, false),
            (Some(deleted.clone()), false, true),
            (Some(deleted), true, false),
            (Some(reconnected), false, false),
        ] {
            let db = MockDatabase::new(DatabaseBackend::Postgres)
                .append_query_results([stored.into_iter().collect::<Vec<_>>()])
                .into_connection();
            assert_eq!(
                Model::supersedes(&db, &fixture.event, deletion)
                    .await
                    .unwrap(),
                expected
            );
        }
    }

    #[tokio::test]
    async fn missing_activity_id_fails_before_query_or_write() {
        let fixture = Fixture::new();
        let mut event = fixture.event.clone();
        event.strava_activity_id = None;
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        assert_eq!(
            event.activity_id().unwrap_err().status,
            axum::http::StatusCode::BAD_REQUEST
        );
        assert!(Model::matches_content(&db, &event, "fixture-content")
            .await
            .is_err());
        assert!(Model::supersedes(&db, &event, false).await.is_err());
        assert!(Model::apply_event(&db, &event, None, false).await.is_err());
        assert!(db.into_transaction_log().is_empty());
    }

    #[tokio::test]
    async fn query_failure_is_an_error_instead_of_an_absent_state() {
        let fixture = Fixture::new();
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_errors((0..2).map(|_| DbErr::Custom("Fixture query failure".into())))
            .into_connection();
        assert!(
            Model::matches_content(&db, &fixture.event, "fixture-content")
                .await
                .is_err()
        );
        assert!(Model::supersedes(&db, &fixture.event, false).await.is_err());
    }
}
