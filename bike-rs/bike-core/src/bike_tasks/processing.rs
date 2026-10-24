use super::*;
use crate::{
    entities::strava_events::{ActivityChange, ActivityStatus},
    test_support::strava_fixture::Fixture,
};

#[test]
fn processed_strava_sources_supply_the_owned_identity_to_the_normal_dag() {
    let fixture = Fixture::new();
    let normalized = fixture.normalized_activity();
    let source = ActivityProcessingSource::Strava(Box::new(crate::entities::strava_events::RetainedActivity {
        event: normalized.event,
        source: normalized.source,
    }));
    let result = ActivityProcessingResult::Strava(Box::new(ActivityChange::applied(
        fixture.event.clone(),
        fixture.activity().id,
        18,
        fixture.activity().started_at.date_naive(),
    )));
    source.validate_result(&result).unwrap();
    let task = result.activity().unwrap().unwrap();
    assert_eq!(task.user_id, fixture.event.bike_user_id);
    assert_eq!(Some(task.activity_id), Some(fixture.activity().id));
}

#[test]
fn skipped_provider_sources_do_not_schedule_derived_work_and_incomplete_success_is_rejected() {
    let fixture = Fixture::new();
    for status in [ActivityStatus::Superseded, ActivityStatus::SourceChanged] {
        assert!(ActivityProcessingResult::Strava(Box::new(fixture.change(status)))
            .activity()
            .unwrap()
            .is_none());
    }
    let incomplete = ActivityChange::unchanged(fixture.event.clone(), ActivityStatus::Unchanged);
    assert!(ActivityProcessingResult::Strava(Box::new(incomplete))
        .activity()
        .is_err());
}

#[test]
fn a_provider_result_cannot_schedule_another_riders_activity() {
    let fixture = Fixture::new();
    let normalized = fixture.normalized_activity();
    let source = ActivityProcessingSource::Strava(Box::new(crate::entities::strava_events::RetainedActivity {
        event: normalized.event,
        source: normalized.source,
    }));
    let mut change = fixture.change(ActivityStatus::Applied);
    change.event.bike_user_id += 1;
    assert!(source
        .validate_result(&ActivityProcessingResult::Strava(Box::new(change)))
        .is_err());
}
