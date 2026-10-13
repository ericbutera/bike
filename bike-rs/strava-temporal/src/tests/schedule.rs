use super::{fixture::Fixture, logging::Logs, runtime::WorkflowMock};
use crate::activities::StravaActivities;
use crate::workflows::{ScheduledSync, SyncActivities};
use bike_core::strava::{events::StravaOperation, native::*};
use temporalio_workflow::common::WorkflowDefinition;

#[test]
fn new_scheduled_run_uses_its_temporal_run_id_as_the_root_correlation() {
    let mut workflow = WorkflowMock::new::<ScheduledSync>(None);
    let request = ScanRequest {
        correlation_id: uuid::Uuid::parse_str("00000000-0000-4000-8000-000000000099").unwrap(),
        after_connection_id: 0,
        failed_customers: 0,
    };
    workflow.activity(
        StravaActivities::scan_connections.name(),
        &request,
        &ScanPage {
            events: vec![],
            next_connection_id: None,
        },
    );
    workflow.result::<()>();
}

#[test]
fn scheduled_sync_starts_and_awaits_each_customer() {
    let fixture = Fixture::new(StravaOperation::Sync);
    let request = request();
    let mut workflow = WorkflowMock::new::<ScheduledSync>(Some(request.clone()));
    workflow.activity(
        StravaActivities::scan_connections.name(),
        &request,
        &ScanPage {
            events: vec![fixture.event.clone()],
            next_connection_id: None,
        },
    );
    let child = workflow.start_child(SyncActivities::run.name(), &fixture.event);
    workflow.complete_child(
        child,
        &CollectionResult {
            event: fixture.event,
            imported: 1,
            unchanged: 0,
            last_started_at: None,
        },
    );
    workflow.result::<()>();
}

#[test]
fn failed_customer_does_not_prevent_later_customers_or_scan_pages() {
    let mut fixture = Fixture::new(StravaOperation::Sync);
    let request = request();
    fixture.event.correlation_id = request.correlation_id;
    let mut other = fixture.event.clone();
    other.bike_user_id = 8;
    other.bike_customer_id = uuid::Uuid::from_u128(8);
    other.connection_id = 10;
    other.strava_athlete_id = 124;
    other.strava_event_id = uuid::Uuid::from_u128(3);
    let other_id = other.workflow_id();
    let logs = Logs::default();
    logs.capture(|| {
        let mut workflow = WorkflowMock::new::<ScheduledSync>(Some(request.clone()));
        workflow.activity(
            StravaActivities::scan_connections.name(),
            &request,
            &ScanPage {
                events: vec![fixture.event.clone(), other.clone()],
                next_connection_id: Some(20),
            },
        );
        let failed = workflow.start_child(SyncActivities::run.name(), &fixture.event);
        let succeeded = workflow.start_child(SyncActivities::run.name(), &other);
        workflow.fail_child(failed);
        workflow.complete_child(
            succeeded,
            &CollectionResult {
                event: other,
                imported: 1,
                unchanged: 0,
                last_started_at: None,
            },
        );
        let continued = workflow.continued_input::<Option<ScanRequest>>().unwrap();
        assert_eq!(continued.failed_customers, 1);
        assert_eq!(continued.after_connection_id, 20);
        assert_eq!(continued.correlation_id, request.correlation_id);
    });
    let failed = logs.workflow(&fixture.event.workflow_id(), "failed");
    let succeeded = logs.workflow(&other_id, "completed");
    for row in [failed, succeeded] {
        assert_eq!(
            row["span"]["correlation_id"],
            request.correlation_id.to_string()
        );
        assert_eq!(row["span"]["parent_temporal_workflow_id"], "unit-workflow");
        assert_eq!(
            row["span"]["parent_temporal_run_id"],
            "00000000-0000-4000-8000-000000000099"
        );
        assert_eq!(row["span"]["request_id"], "fixture-request");
        assert_eq!(row["span"]["trace_id"], "12345678901234567890123456789012");
    }
    assert_eq!(
        logs.workflow("unit-workflow", "continued")["span"]["correlation_id"],
        request.correlation_id.to_string()
    );
}

#[test]
fn last_scan_page_reports_prior_customer_failures() {
    let mut request = request();
    request.failed_customers = 2;
    let mut workflow = WorkflowMock::new::<ScheduledSync>(Some(request.clone()));
    workflow.activity(
        StravaActivities::scan_connections.name(),
        &request,
        &ScanPage {
            events: vec![],
            next_connection_id: None,
        },
    );
    workflow.assert_failed();
}

fn request() -> ScanRequest {
    ScanRequest {
        correlation_id: uuid::Uuid::from_u128(99),
        after_connection_id: 0,
        failed_customers: 0,
    }
}
