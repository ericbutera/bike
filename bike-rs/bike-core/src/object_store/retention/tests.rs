use super::*;
use crate::test_support;

fn record(lifetime: ObjectLifetime) -> RetentionRecord {
    RetentionRecord {
        object: ObjectRef {
            path: "bike/tasks/7/source/hash.json".into(),
            sha256: "hash".into(),
            bytes: 12,
        },
        owner: ObjectOwner::BikeTasks,
        lifetime,
    }
}

fn complete_inventory() -> ReferenceInventory {
    ReferenceInventory {
        complete: true,
        ..Default::default()
    }
}

fn decision(
    record: &RetentionRecord,
    after: TimeDelta,
    inventory: ReferenceInventory,
) -> RetentionDecision {
    record.decision(
        ObjectOwner::BikeTasks,
        test_support::timestamp() + after,
        &RetentionPolicy::default(),
        inventory,
    )
}

#[test]
fn abandoned_inputs_expire_from_the_last_retry() {
    let at = test_support::timestamp();
    let pending = record(ObjectLifetime::Unaccepted {
        last_attempt_at: at,
    });
    assert_eq!(
        decision(
            &pending,
            TimeDelta::hours(24) - TimeDelta::seconds(1),
            complete_inventory()
        ),
        RetentionDecision::NotExpired
    );
    assert_eq!(
        decision(&pending, TimeDelta::hours(24), complete_inventory()),
        RetentionDecision::Candidate
    );
    let retried = record(ObjectLifetime::Unaccepted {
        last_attempt_at: at + TimeDelta::hours(23),
    });
    assert_eq!(
        decision(&retried, TimeDelta::hours(24), complete_inventory()),
        RetentionDecision::NotExpired
    );
}

#[test]
fn closed_chain_outcomes_use_distinct_ttls_and_persist_without_history() {
    for (outcome, days) in [
        (Outcome::Succeeded, 14),
        (Outcome::Failed, 90),
        (Outcome::Canceled, 90),
    ] {
        let saved = record(ObjectLifetime::Closed {
            final_chain_closed_at: test_support::timestamp(),
            outcome,
        });
        let saved: RetentionRecord =
            serde_json::from_value(serde_json::to_value(saved).unwrap()).unwrap();
        let ttl = TimeDelta::days(days);
        assert_eq!(
            decision(&saved, ttl - TimeDelta::seconds(1), complete_inventory()),
            RetentionDecision::NotExpired
        );
        assert_eq!(
            decision(&saved, ttl, complete_inventory()),
            RetentionDecision::Candidate
        );
    }
}

#[test]
fn canonical_and_open_objects_never_become_ttl_candidates() {
    for lifetime in [ObjectLifetime::Canonical, ObjectLifetime::Open] {
        assert_eq!(
            decision(
                &record(lifetime),
                TimeDelta::days(3650),
                complete_inventory()
            ),
            RetentionDecision::Protected
        );
    }
}

#[test]
fn missing_inventory_and_each_reference_protect_expired_objects() {
    let expired = record(ObjectLifetime::Unaccepted {
        last_attempt_at: test_support::timestamp(),
    });
    assert_eq!(
        decision(
            &expired,
            TimeDelta::days(100),
            ReferenceInventory::default()
        ),
        RetentionDecision::InventoryUnavailable
    );
    for inventory in [
        ReferenceInventory {
            workflow_reference: true,
            ..complete_inventory()
        },
        ReferenceInventory {
            database_reference: true,
            ..complete_inventory()
        },
        ReferenceInventory {
            retry_pin: true,
            ..complete_inventory()
        },
    ] {
        assert_eq!(
            decision(&expired, TimeDelta::days(100), inventory),
            RetentionDecision::Protected
        );
    }
}

#[test]
fn sweeper_owner_and_path_must_both_match() {
    let expired = record(ObjectLifetime::Unaccepted {
        last_attempt_at: test_support::timestamp(),
    });
    assert_eq!(
        expired.decision(
            ObjectOwner::Strava,
            test_support::timestamp() + TimeDelta::days(100),
            &RetentionPolicy::default(),
            complete_inventory()
        ),
        RetentionDecision::ForeignOwner
    );
    for path in [
        "strava/7/raw/hash.json",
        "bike/tasks-other/7/hash.json",
        "bike/tasks/../strava/hash.json",
    ] {
        let foreign = RetentionRecord {
            object: ObjectRef {
                path: path.into(),
                ..expired.object.clone()
            },
            ..expired.clone()
        };
        assert_eq!(
            decision(&foreign, TimeDelta::days(100), complete_inventory()),
            RetentionDecision::ForeignOwner
        );
    }
}

#[test]
fn ttl_configuration_requires_positive_durations_and_controls_expiry() {
    let hour = TimeDelta::hours(1);
    assert!(RetentionPolicy::new(TimeDelta::zero(), hour, hour).is_err());
    let policy = RetentionPolicy::new(hour, hour * 2, hour * 3).unwrap();
    let expired = record(ObjectLifetime::Unaccepted {
        last_attempt_at: test_support::timestamp(),
    });
    assert_eq!(
        expired.decision(
            ObjectOwner::BikeTasks,
            test_support::timestamp() + hour,
            &policy,
            complete_inventory()
        ),
        RetentionDecision::Candidate
    );
}
