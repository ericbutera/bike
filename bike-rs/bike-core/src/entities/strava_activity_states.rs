use crate::entities::strava_events::{SourceVersion, StravaEvent};
use sea_orm::entity::prelude::*;
use sea_orm::{sea_query::OnConflict, ConnectionTrait, Set};

/// Latest applied state for one remote (athlete/owner, activity/object) pair.
/// This is the stale-event fence and remote-to-Bike identity mapping, not history.
/// event_time retains the source timestamp (Strava's clock for webhooks).
/// updated_at/applied_at use Bike's clock and do not decide event ordering.
/// Historical changes belong to activity_event_history in the same transaction.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[sea_orm(table_name = "strava_activity_states")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub athlete_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i64,
    /// Historical Bike PK; retain it when the activity is deleted.
    pub bike_activity_id: Option<i32>,
    pub connection_generation: DateTimeUtc,
    /// Greatest accepted provider timestamp, or zero when none is known.
    pub event_time: i64,
    pub applied_hash: Option<String>,
    pub deleted: bool,
    /// Source event timestamp for the current deletion; cleared by a newer upsert.
    pub deleted_at: Option<DateTimeUtc>,
    /// Most recent local state write. Never use it to order Strava events.
    pub updated_at: DateTimeUtc,
    /// Compatibility with the deployed worker; ordering uses event_time.
    pub applied_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
impl Model {
    pub async fn for_bike_activity(
        db: &impl ConnectionTrait,
        connection: &super::strava_connections::Model,
        activity_id: i32,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::AthleteId.eq(connection.athlete_id))
            .filter(Column::ConnectionGeneration.eq(connection.created_at))
            .filter(Column::BikeActivityId.eq(activity_id))
            .filter(Column::Deleted.eq(false))
            .one(db)
            .await
    }

    pub async fn capture_version(db: &impl ConnectionTrait, event: &StravaEvent) -> Result<SourceVersion, DbErr> {
        Ok(SourceVersion {
            updated_at: Entity::find_by_id(Self::key_for_event(event)?)
                .one(db)
                .await?
                .map(|state| state.updated_at),
        })
    }

    pub async fn matches_version(db: &impl ConnectionTrait, event: &StravaEvent) -> Result<bool, DbErr> {
        let Some(expected) = &event.source_version else {
            // Retained workflow histories predate source snapshots.
            return Ok(true);
        };
        Ok(*expected == Self::capture_version(db, event).await?)
    }

    pub async fn matches_content(db: &impl ConnectionTrait, event: &StravaEvent, hash: &str) -> Result<bool, DbErr> {
        Self::matches_by_id(db, event, |state| state.matches_event_content(event, hash)).await
    }
    pub async fn supersedes(db: &impl ConnectionTrait, event: &StravaEvent, deletion: bool) -> Result<bool, DbErr> {
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
        let id = event.activity_id().map_err(|error| DbErr::Custom(error.message))?;
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
        let Some(provider_time) = event.provider_event_time() else {
            // A local sync cannot undo a provider tombstone or advance its clock.
            return self.deleted && !deletion;
        };
        self.event_time > provider_time || self.is_deletion_at(provider_time, deletion)
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
    ) -> Result<Self, DbErr> {
        Self::from_event(event, hash, deleted)?.save(db).await
    }

    fn from_event(event: &StravaEvent, hash: Option<String>, deleted: bool) -> Result<Self, DbErr> {
        let (athlete_id, activity_id) = Self::key_for_event(event)?;
        let deleted_at = event
            .provider_event_time()
            .filter(|_| deleted)
            .map(|at| {
                chrono::DateTime::from_timestamp(at, 0)
                    .ok_or_else(|| DbErr::Custom("Invalid Strava deletion event time".into()))
            })
            .transpose()?;
        let now = chrono::Utc::now();
        Ok(Self {
            athlete_id,
            activity_id,
            bike_activity_id: event.bike_activity_id,
            connection_generation: event.connection_generation,
            event_time: event.provider_event_time().unwrap_or(0),
            applied_hash: hash,
            deleted,
            deleted_at,
            updated_at: now,
            applied_at: now,
        })
    }
    async fn save(self, db: &impl ConnectionTrait) -> Result<Self, DbErr> {
        let updates = vec![
            Column::ConnectionGeneration,
            Column::AppliedHash,
            Column::Deleted,
            Column::DeletedAt,
        ];
        Entity::insert(ActiveModel {
            athlete_id: Set(self.athlete_id),
            activity_id: Set(self.activity_id),
            bike_activity_id: Set(self.bike_activity_id),
            connection_generation: Set(self.connection_generation),
            event_time: Set(self.event_time),
            applied_hash: Set(self.applied_hash),
            deleted: Set(self.deleted),
            deleted_at: Set(self.deleted_at),
            updated_at: Set(self.updated_at),
            applied_at: Set(self.applied_at),
        })
        .on_conflict(
            OnConflict::columns([Column::AthleteId, Column::ActivityId])
                .update_columns(updates)
                .value(
                    Column::BikeActivityId,
                    Expr::cust(
                        "CASE WHEN strava_activity_states.connection_generation = EXCLUDED.connection_generation \
                     THEN COALESCE(EXCLUDED.bike_activity_id, strava_activity_states.bike_activity_id) \
                     ELSE EXCLUDED.bike_activity_id END",
                    ),
                )
                .value(
                    Column::EventTime,
                    Expr::cust(
                        "CASE WHEN strava_activity_states.connection_generation = EXCLUDED.connection_generation \
                     THEN GREATEST(strava_activity_states.event_time, EXCLUDED.event_time) \
                     ELSE EXCLUDED.event_time END",
                    ),
                )
                .value(Column::UpdatedAt, Self::next_write_version())
                .value(Column::AppliedAt, Self::next_write_version())
                .to_owned(),
        )
        .exec_with_returning(db)
        .await
    }

    fn next_write_version() -> Expr {
        Expr::cust("GREATEST(strava_activity_states.updated_at + INTERVAL '1 microsecond', EXCLUDED.updated_at)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::strava_fixture::Fixture;
    use sea_orm::{DatabaseBackend, MockDatabase};

    #[test]
    fn applied_and_deleted_events_keep_bike_and_remote_activity_identity() {
        let fixture = Fixture::new();
        let event = StravaEvent {
            bike_activity_id: Some(fixture.activity().id),
            ..fixture.event.clone()
        };
        for deleted in [false, true] {
            let state = Model::from_event(&event, None, deleted).unwrap();
            assert_eq!(state.bike_activity_id, Some(fixture.activity().id));
            assert_eq!(state.athlete_id, event.strava_athlete_id);
            assert_eq!(state.activity_id, event.strava_activity_id.unwrap());
            assert_eq!(state.deleted, deleted);
            assert_eq!(
                state.deleted_at.map(|at| at.timestamp()),
                deleted.then_some(event.event_time)
            );
            assert_eq!(state.updated_at, state.applied_at);
        }
    }

    fn state(fixture: &Fixture) -> Model {
        Model {
            applied_hash: Some("fixture-content".into()),
            ..fixture.activity_state()
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
                Model::matches_content(&db, &fixture.event, hash).await.unwrap(),
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
                Model::supersedes(&db, &fixture.event, deletion).await.unwrap(),
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
        assert!(Model::matches_content(&db, &event, "fixture-content").await.is_err());
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
        assert!(Model::matches_content(&db, &fixture.event, "fixture-content")
            .await
            .is_err());
        assert!(Model::supersedes(&db, &fixture.event, false).await.is_err());
    }

    #[tokio::test]
    async fn local_sync_clock_never_supersedes_a_provider_event_or_restores_a_tombstone() {
        let fixture = Fixture::new();
        let mut stored = state(&fixture);
        let mut event = fixture.event.clone();
        event.webhook = None;
        for local_time in [stored.event_time - 3600, stored.event_time + 3600] {
            event.event_time = local_time;
            assert!(!stored.supersedes_event(&event, false));
            assert_eq!(Model::from_event(&event, None, false).unwrap().event_time, 0);
        }
        stored.deleted = true;
        assert!(stored.supersedes_event(&event, false));
        assert!(!stored.supersedes_event(&event, true));
        assert_eq!(Model::from_event(&event, None, true).unwrap().deleted_at, None);
    }

    #[tokio::test]
    async fn captured_source_version_fences_changed_or_newly_created_rows() {
        let fixture = Fixture::new();
        let stored = state(&fixture);
        for (expected, current, matches) in [
            (None, None, true),
            (None, Some(stored.clone()), false),
            (Some(stored.updated_at), Some(stored.clone()), true),
            (
                Some(stored.updated_at - chrono::Duration::microseconds(1)),
                Some(stored.clone()),
                false,
            ),
            (Some(stored.updated_at), None, false),
        ] {
            let event = StravaEvent {
                source_version: Some(SourceVersion { updated_at: expected }),
                ..fixture.event.clone()
            };
            let db = MockDatabase::new(DatabaseBackend::Postgres)
                .append_query_results([current.into_iter().collect::<Vec<_>>()])
                .into_connection();
            assert_eq!(Model::matches_version(&db, &event).await.unwrap(), matches);
        }
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        assert!(Model::matches_version(&db, &fixture.event).await.unwrap());
        assert!(db.into_transaction_log().is_empty());
    }

    #[tokio::test]
    async fn state_write_preserves_provider_watermark_and_advances_the_existing_write_token() {
        let fixture = Fixture::new();
        let event = StravaEvent {
            webhook: None,
            event_time: i64::MAX,
            ..fixture.event.clone()
        };
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![state(&fixture)]])
            .into_connection();
        Model::apply_event(&db, &event, None, false).await.unwrap();
        let log = db.into_transaction_log();
        let statement = &log[0].statements()[0];
        assert!(statement
            .sql
            .contains("GREATEST(strava_activity_states.event_time, EXCLUDED.event_time)"));
        assert!(statement
            .sql
            .contains("connection_generation = EXCLUDED.connection_generation"));
        assert_eq!(
            statement.sql.matches("updated_at + INTERVAL '1 microsecond'").count(),
            2
        );
        assert!(!statement.values.as_ref().unwrap().0.contains(&i64::MAX.into()));
    }

    #[test]
    fn provider_time_orders_deletion_while_distinct_equal_second_updates_reconcile() {
        let fixture = Fixture::new();
        let mut state = state(&fixture);
        let mut event = fixture.event;
        event.event_time = i64::MAX;
        event.webhook.as_mut().unwrap().event_time = state.event_time - 1;
        assert!(state.supersedes_event(&event, false));
        event.event_time = i64::MIN;
        event.webhook.as_mut().unwrap().event_time = state.event_time;
        assert!(!state.supersedes_event(&event, false));
        event.webhook.as_mut().unwrap().updates = Some(serde_json::json!({"title":"Distinct same-second update"}));
        assert!(!state.supersedes_event(&event, false));
        state.deleted = true;
        assert!(state.supersedes_event(&event, false));
        event.webhook.as_mut().unwrap().event_time += 1;
        assert!(!state.supersedes_event(&event, false));
        event.connection_generation += chrono::Duration::microseconds(1);
        assert!(!state.supersedes_event(&event, false));
    }
}
