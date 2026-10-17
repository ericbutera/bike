use super::handlers_fixture::{HandlerFixture, ProviderCall};
use super::*;
use crate::strava::{objects::ObjectRef, provider_payload::StoredStravaProviderPayload};
use axum::http::StatusCode;
use bike_core::activity_parser::ParsedActivityData;

#[tokio::test]
async fn non_cycling_summary_clears_old_cycling_training_analysis_without_touching_rides() {
    let fixture = super::fixture::Fixture::new();
    for sport in ["walk", "ride"] {
        let mut activity = fixture.activity();
        activity.sport = sport.into();
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_exec_results([sea_orm::MockExecResult {
                rows_affected: 1,
                last_insert_id: 0,
            }])
            .into_connection();
        bike_core::entities::activity_training_analyses::Model::clear_for_non_cycling(&db, &activity)
            .await
            .unwrap();
        let log = db.into_transaction_log();
        if sport == "walk" {
            assert_eq!(log.len(), 1);
            assert!(log[0].statements()[0]
                .sql
                .contains("DELETE FROM \"activity_training_analyses\""));
        } else {
            assert!(log.is_empty());
        }
    }
}

#[tokio::test]
async fn fetches_fixture_activity_and_streams_with_mocked_ownership_and_storage() {
    let fixture = HandlerFixture::new();
    let db = fixture
        .connection_mock()
        .append_query_results([vec![fixture.domain.connection.clone()]])
        .append_query_results([vec![fixture.domain.customer.clone()]]);
    let operations = fixture.operations(db);
    let activity = operations.fetch_activity(fixture.domain.event.clone()).await.unwrap();
    let sources = operations.fetch_streams(activity).await.unwrap();
    let summary: StravaActivitySummary = operations.store.read_json(&sources.activity).await.unwrap();
    let streams: StravaActivityStreams = operations.store.read_json(&sources.streams).await.unwrap();
    assert_eq!(summary.id, fixture.domain.event.strava_activity_id.unwrap());
    assert_eq!(sources.event.correlation_id, fixture.domain.event.correlation_id);
    assert_eq!(
        streams.time.unwrap().data,
        fixture.provider.payload.streams.time.clone().unwrap().data
    );
    assert_eq!(
        *fixture.provider.calls.lock().unwrap(),
        [ProviderCall::Activity, ProviderCall::Streams]
    );
}

#[tokio::test]
async fn multi_megabyte_provider_data_stays_in_storage_across_workflow_boundaries() {
    let mut fixture = HandlerFixture::new();
    let extension = "a".repeat(3 * 1024 * 1024);
    Arc::get_mut(&mut fixture.provider)
        .unwrap()
        .payload
        .activity
        .additional_fields
        .insert("provider_extension".into(), serde_json::json!(extension));
    let db = fixture
        .connection_mock()
        .append_query_results([vec![fixture.domain.connection.clone()]])
        .append_query_results([vec![fixture.domain.customer.clone()]]);
    let operations = fixture.operations(db);
    let source = operations.fetch_activity(fixture.domain.event.clone()).await.unwrap();
    assert!(source.activity.bytes > 2 * 1024 * 1024);
    crate::strava::payload::validate_temporal(&source).unwrap();
    let sources = operations.fetch_streams(source).await.unwrap();
    crate::strava::payload::validate_temporal(&sources).unwrap();
    let normalized = operations.normalize_activity(sources).await.unwrap();
    crate::strava::payload::validate_temporal(&normalized).unwrap();
    let retained: StoredStravaProviderPayload = operations.store.read_json(&normalized.source).await.unwrap();
    assert_eq!(retained.activity.additional_fields["provider_extension"], extension);
    assert!(normalized.parsed.is_some());
    assert!(serde_json::to_vec(&normalized).unwrap().len() < 4096);
}

#[tokio::test]
async fn normalizes_a_fixture_ride_without_provider_or_persistence_calls() {
    let fixture = HandlerFixture::new();
    let operations = fixture.operations(MockDatabase::new(DatabaseBackend::Postgres));
    let normalized = operations.normalize_activity(fixture.sources().await).await.unwrap();
    let saved: StoredStravaProviderPayload = operations.store.read_json(&normalized.source).await.unwrap();
    let parsed: ParsedActivityData = operations
        .store
        .read_json(normalized.parsed.as_ref().unwrap())
        .await
        .unwrap();
    assert_eq!(saved.activity.id, fixture.provider.payload.activity.id);
    assert_eq!(parsed.draft.title, fixture.provider.payload.activity.name);
    assert!(parsed.draft.distance_meters.unwrap() > 0.0);
    assert_eq!(normalized.event.strava_event_id, fixture.domain.event.strava_event_id);
    assert!(fixture.provider.calls.lock().unwrap().is_empty());
    assert!(operations.db.into_transaction_log().is_empty());
}

#[tokio::test]
async fn non_cycling_activity_keeps_a_normalized_summary_without_provider_streams() {
    let mut fixture = HandlerFixture::new();
    let provider = Arc::get_mut(&mut fixture.provider).unwrap();
    provider.payload.activity.sport_type = Some("Run".into());
    provider.payload.activity.legacy_type = Some("Run".into());
    let sources = fixture.sources().await;
    let operations = fixture.operations(fixture.connection_mock());
    let source = ActivitySource {
        event: sources.event,
        activity: sources.activity,
    };
    let sources = operations.fetch_streams(source).await.unwrap();
    let empty: StravaActivityStreams = operations.store.read_json(&sources.streams).await.unwrap();
    assert!(empty.time.is_none());
    let normalized = operations.normalize_activity(sources).await.unwrap();
    let parsed: ParsedActivityData = operations
        .store
        .read_json(normalized.parsed.as_ref().unwrap())
        .await
        .unwrap();
    assert_eq!(parsed.draft.sport, "run");
    assert!(parsed.derived_data.route_points.is_empty());
    assert!(fixture.provider.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn manual_cycling_summary_does_not_request_missing_streams() {
    let mut fixture = HandlerFixture::new();
    let provider = Arc::get_mut(&mut fixture.provider).unwrap();
    provider
        .payload
        .activity
        .additional_fields
        .insert("manual".into(), serde_json::json!(true));
    provider
        .payload
        .activity
        .additional_fields
        .insert("total_elevation_gain".into(), serde_json::json!(407.5));
    let sources = fixture.sources().await;
    let operations = fixture.operations(fixture.connection_mock());
    let sources = operations
        .fetch_streams(ActivitySource {
            event: sources.event,
            activity: sources.activity,
        })
        .await
        .unwrap();
    let normalized = operations.normalize_activity(sources).await.unwrap();
    let parsed: ParsedActivityData = operations
        .store
        .read_json(normalized.parsed.as_ref().unwrap())
        .await
        .unwrap();
    assert_eq!(parsed.draft.elevation_gain_meters, Some(407.5));
    assert!(parsed.derived_data.route_points.is_empty());
    assert!(fixture.provider.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn missing_or_mismatched_activity_id_never_advances_the_handler() {
    let fixture = HandlerFixture::new();
    let mut event = fixture.domain.event.clone();
    event.strava_activity_id = None;
    let operations = fixture.operations(MockDatabase::new(DatabaseBackend::Postgres));
    assert!(operations.fetch_activity(event).await.is_err());
    assert!(fixture.provider.calls.lock().unwrap().is_empty());
    let mut event = fixture.domain.event.clone();
    event.strava_activity_id = Some(999);
    let operations = fixture.operations(fixture.connection_mock());
    assert_eq!(
        operations.fetch_activity(event).await.unwrap_err().status,
        StatusCode::BAD_REQUEST
    );
    let mut sources = fixture.sources().await;
    sources.event.strava_activity_id = Some(999);
    assert!(operations.normalize_activity(sources).await.is_err());
}

#[tokio::test]
async fn provider_rate_limit_keeps_retry_time_and_writes_no_source() {
    let fixture = HandlerFixture::new();
    *fixture.provider.failure.lock().unwrap() = Some(StatusCode::TOO_MANY_REQUESTS);
    let error = fixture
        .operations(fixture.connection_mock())
        .fetch_activity(fixture.domain.event.clone())
        .await
        .unwrap_err();
    assert_eq!(error.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(error.retry_at.unwrap().timestamp(), 1790856300);
    assert!(fixture.storage.objects.lock().unwrap().is_empty());
}

#[tokio::test]
async fn storage_and_persistence_failures_prevent_provider_work() {
    let fixture = HandlerFixture::new();
    *fixture.storage.failure.lock().unwrap() = true;
    let source = ActivitySource {
        event: fixture.domain.event.clone(),
        activity: ObjectRef {
            path: "fixture".into(),
            sha256: "fixture".into(),
            bytes: 0,
        },
    };
    let operations = fixture.operations(MockDatabase::new(DatabaseBackend::Postgres));
    assert!(operations.fetch_streams(source).await.is_err());
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_errors([sea_orm::DbErr::Custom("Fixture persistence failed".into())]);
    assert!(fixture
        .operations(db)
        .fetch_activity(fixture.domain.event.clone())
        .await
        .is_err());
    assert!(fixture.provider.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn listing_passes_the_incremental_offset_and_filters_the_backfill_end() {
    let fixture = HandlerFixture::new();
    let at = fixture.provider.payload.activity.start_date.timestamp();
    let input = PageInput {
        collection: CollectionInput {
            event: fixture.domain.event.clone(),
            window: ActivityWindow {
                after: at - 1,
                before: Some(at),
            },
        },
        page: 3,
    };
    let operations = fixture.operations(fixture.connection_mock());
    let page = operations.list_page(input).await.unwrap();
    assert!(page.activity_ids.is_empty());
    assert_eq!(page.next_page, Some(4));
    assert_eq!(
        *fixture.provider.calls.lock().unwrap(),
        [ProviderCall::Page { after: at - 1, page: 3 }]
    );
}

#[tokio::test]
async fn listing_retains_matching_ids_dates_and_stops_on_empty_provider_page() {
    let mut fixture = HandlerFixture::new();
    let input = PageInput {
        collection: CollectionInput {
            event: fixture.domain.event.clone(),
            window: ActivityWindow { after: 0, before: None },
        },
        page: 1,
    };
    let page = fixture
        .operations(fixture.connection_mock())
        .list_page(input.clone())
        .await
        .unwrap();
    assert_eq!(page.activity_ids, [fixture.provider.payload.activity.id]);
    assert_eq!(
        page.latest_started_at,
        Some(fixture.provider.payload.activity.start_date)
    );
    Arc::get_mut(&mut fixture.provider).unwrap().empty_page = true;
    let page = fixture
        .operations(fixture.connection_mock())
        .list_page(input)
        .await
        .unwrap();
    assert_eq!(page.next_page, None);
}

#[tokio::test]
async fn persist_retry_returns_the_committed_mutation_without_reading_storage() {
    let fixture = HandlerFixture::new();
    let mut row = fixture.domain.row(ActivityStatus::Applied);
    row.mutation = Some(serde_json::to_value(fixture.domain.change(ActivityStatus::Applied)).unwrap());
    let db = fixture.connection_mock().append_query_results([vec![row]]);
    let operations = fixture.operations(db);
    *fixture.storage.failure.lock().unwrap() = true;
    let input = NormalizedActivity {
        event: fixture.domain.event.clone(),
        source: ObjectRef {
            path: "unavailable".into(),
            sha256: "fixture".into(),
            bytes: 0,
        },
        parsed: None,
    };
    let change = operations.persist_activity(input).await.unwrap();
    assert_eq!(change.status, ActivityStatus::Applied);
    assert_eq!(operations.db.into_transaction_log().len(), 3);
    assert!(fixture.provider.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn cycling_persistence_requires_normalized_data_before_opening_a_transaction() {
    let fixture = HandlerFixture::new();
    let db = fixture
        .connection_mock()
        .append_query_results([vec![fixture.domain.row(ActivityStatus::Pending)]]);
    let operations = fixture.operations(db);
    let source = fixture
        .storage
        .write("fixture/source", crate::strava::fixtures::REAL_RIDE)
        .await
        .unwrap();
    let input = NormalizedActivity {
        event: fixture.domain.event.clone(),
        source,
        parsed: None,
    };
    assert_eq!(
        operations.persist_activity(input).await.unwrap_err().status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(operations.db.into_transaction_log().len(), 3);
}

#[tokio::test]
async fn source_deletion_is_idempotent_and_propagates_storage_failure() {
    let fixture = HandlerFixture::new();
    let operations = fixture.operations(MockDatabase::new(DatabaseBackend::Postgres));
    let mut change = fixture.domain.change(ActivityStatus::Deleted);
    change.removed_source = Some("fixture/source".into());
    operations.delete_source(change.clone()).await.unwrap();
    operations.delete_source(change.clone()).await.unwrap();
    assert_eq!(
        *fixture.storage.removed.lock().unwrap(),
        ["fixture/source", "fixture/source"]
    );
    *fixture.storage.failure.lock().unwrap() = true;
    assert!(operations.delete_source(change).await.is_err());
    assert!(operations.db.into_transaction_log().is_empty());
}

#[tokio::test]
async fn remote_revocation_skips_webhooks_accepts_already_revoked_and_propagates_other_errors() {
    let fixture = HandlerFixture::new();
    let operations = fixture.operations(MockDatabase::new(DatabaseBackend::Postgres));
    operations
        .revoke_connection(fixture.domain.event.clone())
        .await
        .unwrap();
    assert!(fixture.provider.calls.lock().unwrap().is_empty());
    let mut event = fixture.domain.event.clone();
    event.revoke_remote = true;
    let operations = fixture.operations(fixture.connection_mock());
    operations.revoke_connection(event.clone()).await.unwrap();
    *fixture.provider.failure.lock().unwrap() = Some(StatusCode::UNAUTHORIZED);
    fixture
        .operations(fixture.connection_mock())
        .revoke_connection(event.clone())
        .await
        .unwrap();
    *fixture.provider.failure.lock().unwrap() = Some(StatusCode::BAD_GATEWAY);
    assert!(fixture
        .operations(fixture.connection_mock())
        .revoke_connection(event)
        .await
        .is_err());
}
