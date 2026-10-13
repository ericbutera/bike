//! The retained Strava object layout. Keep these keys stable across code moves.
use super::objects::checksum;
use crate::entities::{strava_connections, strava_workflows};

pub const ROOT: &str = "strava";
pub const SOURCES: &str = "strava/sources/";

pub(crate) struct WorkflowPaths<'a> {
    row: &'a strava_workflows::Model,
}

impl<'a> WorkflowPaths<'a> {
    pub(crate) fn new(row: &'a strava_workflows::Model) -> Self {
        Self { row }
    }

    pub(crate) fn directory(&self) -> String {
        format!("{ROOT}/{}/runs/{}", self.row.athlete_id, self.row.id)
    }

    pub(crate) fn raw(&self, revision: &str, resource: &str) -> String {
        match self.row.activity_id {
            Some(id) => activity_raw(self.row.athlete_id, id, revision, resource),
            None => format!("{}/raw/{resource}", self.directory()),
        }
    }

    pub(crate) fn transform(&self, stage: &str, input: &str) -> String {
        let subject = self
            .row
            .activity_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| format!("runs/{}", self.row.id));
        format!(
            "{ROOT}/{}/{subject}/transforms/{stage}/v1/{}/workflows/{}",
            self.row.athlete_id,
            checksum(input.as_bytes()),
            self.row.id
        )
    }
}

pub(crate) fn activity_raw(athlete: i64, activity: i64, revision: &str, resource: &str) -> String {
    format!("{ROOT}/{athlete}/{activity}/raw/{revision}/{resource}")
}

pub(crate) fn activity_raw_directory(athlete: i64, activity: i64) -> String {
    format!("{ROOT}/{athlete}/{activity}/raw")
}

pub(crate) fn activity_transform(athlete: i64, activity: i64, stage: &str, input: &str) -> String {
    let prefix = format!("{ROOT}/{athlete}/{activity}/transforms/{stage}/v1");
    if input.is_empty() {
        prefix
    } else {
        format!("{prefix}/{input}")
    }
}

pub(crate) fn sources(user: i32) -> String {
    format!("{SOURCES}{user}")
}

pub(crate) fn authorization(code: &str) -> String {
    format!(
        "{ROOT}/credentials/authorization/{}",
        checksum(code.as_bytes())
    )
}

pub(crate) fn refresh(connection: &strava_connections::Model, token: &str) -> String {
    let identity = format!("{token}:{}", connection.expires_at.timestamp());
    format!(
        "{ROOT}/{}/credentials/{}/refresh/{}",
        connection.athlete_id,
        connection.created_at.timestamp_micros(),
        checksum(identity.as_bytes())
    )
}

pub(crate) fn exchange(archive: Option<&str>, id: &str) -> String {
    match archive {
        Some(archive) => format!("{archive}/exchanges/{id}"),
        None => format!("{ROOT}/exchanges/{id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_keys_keep_activity_and_collection_sources_separate() {
        let mut row = strava_workflows::Model {
            id: "run".into(),
            connection_id: 1,
            athlete_id: 99,
            activity_id: None,
            action: "sync".into(),
            source: serde_json::json!({}),
            checkpoint: serde_json::json!({}),
            status: "accepted".into(),
            error: None,
            accepted_at: chrono::Utc::now(),
            completed_at: None,
        };
        let paths = WorkflowPaths::new(&row);
        assert_eq!(paths.directory(), "strava/99/runs/run");
        assert_eq!(paths.raw("", "pages/1"), "strava/99/runs/run/raw/pages/1");
        assert_eq!(
            paths.transform("discover", "input"),
            format!(
                "strava/99/runs/run/transforms/discover/v1/{}/workflows/run",
                checksum(b"input")
            )
        );
        row.activity_id = Some(42);
        let paths = WorkflowPaths::new(&row);
        assert_eq!(
            paths.raw("revision", "streams"),
            "strava/99/42/raw/revision/streams"
        );
        assert_eq!(
            paths.transform("apply", "input"),
            format!(
                "strava/99/42/transforms/apply/v1/{}/workflows/run",
                checksum(b"input")
            )
        );
        assert_eq!(
            activity_transform(99, 42, "excluded-streams", ""),
            "strava/99/42/transforms/excluded-streams/v1"
        );
    }

    #[test]
    fn activity_keys_preserve_the_existing_layout() {
        assert_eq!(
            activity_raw(99, 42, "event-1", "activity"),
            "strava/99/42/raw/event-1/activity"
        );
        assert_eq!(activity_raw_directory(99, 42), "strava/99/42/raw");
        assert_eq!(
            activity_transform(99, 42, "listed-activity", "hash"),
            "strava/99/42/transforms/listed-activity/v1/hash"
        );
        assert_eq!(sources(7), "strava/sources/7");
        assert_eq!(exchange(None, "request"), "strava/exchanges/request");
        assert_eq!(
            exchange(Some("retained"), "request"),
            "retained/exchanges/request"
        );
        assert_eq!(
            authorization("code"),
            format!("strava/credentials/authorization/{}", checksum(b"code"))
        );
    }
}
