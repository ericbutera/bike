use crate::workflow_error::WorkflowError;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// The operation accepted by a Strava workflow and its durable worker processor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    #[serde(rename = "insert")]
    Create,
    #[serde(rename = "update")]
    Update,
    #[serde(rename = "upsert")]
    Upsert,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "sync")]
    Sync,
    #[serde(rename = "backfill")]
    Backfill,
    #[serde(rename = "deauthorize")]
    Disconnect,
}

impl Action {
    pub const ALL: [Self; 7] = [
        Self::Create,
        Self::Update,
        Self::Upsert,
        Self::Delete,
        Self::Sync,
        Self::Backfill,
        Self::Disconnect,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "insert",
            Self::Update => "update",
            Self::Upsert => "upsert",
            Self::Delete => "delete",
            Self::Sync => "sync",
            Self::Backfill => "backfill",
            Self::Disconnect => "deauthorize",
        }
    }

    pub const fn task_type(self) -> &'static str {
        match self {
            Self::Create => "strava_activity_create",
            Self::Update => "strava_activity_update",
            Self::Upsert => "strava_activity_upsert",
            Self::Delete => "strava_activity_delete",
            Self::Sync => "strava_sync",
            Self::Backfill => "strava_backfill",
            Self::Disconnect => "strava_disconnect",
        }
    }

    pub const fn stages(self) -> &'static [&'static str] {
        match self {
            Self::Create | Self::Update => &["activity", "streams", "apply", "rebuild"],
            Self::Upsert => &["activity", "streams", "apply"],
            Self::Delete => &["capture_delete", "delete", "rebuild"],
            Self::Sync | Self::Backfill => &["discover", "children", "rebuild"],
            Self::Disconnect => &["capture_cleanup", "revoke", "cleanup"],
        }
    }
}

impl FromStr for Action {
    type Err = WorkflowError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|action| action.as_str() == value)
            .ok_or_else(|| WorkflowError::bad_request("Unknown Strava workflow action"))
    }
}

/// Persisted queue metadata remains beside this small, typed domain payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowTask {
    pub workflow_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Execution {
    Completed,
    WaitingForChildren,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operations_have_distinct_processors_and_round_trip_retained_actions() {
        let mut task_types = std::collections::BTreeSet::new();
        for action in Action::ALL {
            assert!(task_types.insert(action.task_type()));
            assert_eq!(action.as_str().parse::<Action>().unwrap(), action);
            assert_eq!(
                serde_json::from_str::<Action>(&serde_json::to_string(&action).unwrap()).unwrap(),
                action
            );
            assert!(!action.stages().is_empty());
        }
        assert!("unknown".parse::<Action>().is_err());
    }

    #[test]
    fn create_and_update_share_upsert_stages_with_their_own_rebuild() {
        assert_eq!(Action::Create.stages(), Action::Update.stages());
        assert_eq!(Action::Upsert.stages(), &["activity", "streams", "apply"]);
        assert_eq!(
            Action::Delete.stages(),
            &["capture_delete", "delete", "rebuild"]
        );
        assert_eq!(Action::Sync.stages(), Action::Backfill.stages());
        assert_eq!(
            Action::Disconnect.stages(),
            &["capture_cleanup", "revoke", "cleanup"]
        );
    }
}
