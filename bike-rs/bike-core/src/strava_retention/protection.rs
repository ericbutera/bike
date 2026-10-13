use crate::{
    entities::strava_workflows,
    strava_objects::{ObjectRef, ObjectStore},
    workflow_error::WorkflowError,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{collections::BTreeSet, path::Path};

#[derive(Default)]
pub(super) struct Protection {
    pub paths: BTreeSet<String>,
    directories: BTreeSet<String>,
    workflow_ids: BTreeSet<String>,
}

impl Protection {
    pub fn contains(&self, path: &str) -> bool {
        let parts: Vec<_> = path.split('/').collect();
        self.paths.contains(path)
            || parts
                .windows(2)
                .any(|pair| pair[0] == "workflows" && self.workflow_ids.contains(pair[1]))
            || Path::new(path).ancestors().skip(1).any(|parent| {
                self.directories
                    .contains(&parent.to_string_lossy().into_owned())
            })
    }

    pub async fn workflows(
        &mut self,
        store: &ObjectStore,
        rows: &[strava_workflows::Model],
        queued: &BTreeSet<String>,
        cutoff: DateTime<Utc>,
    ) -> Result<Vec<String>, WorkflowError> {
        let mut retained: BTreeSet<String> = rows
            .iter()
            .filter(|row| {
                queued.contains(&row.id)
                    || row.status != "completed"
                    || row.completed_at.is_none_or(|at| at >= cutoff)
            })
            .map(|row| row.id.clone())
            .collect();
        loop {
            let count = retained.len();
            for row in rows {
                if let Some(parent) = row.source["parent"].as_str() {
                    if retained.contains(&row.id) || retained.contains(parent) {
                        retained.insert(row.id.clone());
                        retained.insert(parent.to_owned());
                    }
                }
            }
            if retained.len() == count {
                break;
            }
        }
        let mut objects = Vec::new();
        for row in rows.iter().filter(|row| retained.contains(&row.id)) {
            self.workflow_ids.insert(row.id.clone());
            references(&row.source, &mut objects)?;
            references(&row.checkpoint, &mut objects)?;
            // Interrupted stages may have a response/reference on disk without
            // a committed checkpoint yet. Protect this workflow's entire directory.
            self.directories
                .insert(format!("strava/{}/runs/{}", row.athlete_id, row.id));
            if let Some(activity) = row.activity_id {
                if row.status != "completed" {
                    self.directories
                        .insert(format!("strava/{}/{activity}/raw", row.athlete_id));
                }
            }
        }
        self.objects(store, objects).await?;
        Ok(rows
            .iter()
            .filter(|row| !retained.contains(&row.id))
            .map(|row| row.id.clone())
            .collect())
    }

    async fn objects(
        &mut self,
        store: &ObjectStore,
        mut objects: Vec<ObjectRef>,
    ) -> Result<(), WorkflowError> {
        let mut visited = BTreeSet::new();
        while let Some(object) = objects.pop() {
            if !visited.insert((object.path.clone(), object.sha256.clone(), object.bytes)) {
                continue;
            }
            let bytes = store.read(&object).await?;
            let parent = Path::new(&object.path)
                .parent()
                .ok_or_else(|| WorkflowError::internal("Missing Strava object directory"))?;
            self.directories
                .insert(parent.to_string_lossy().into_owned());
            if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                references(&value, &mut objects)?;
            }
        }
        Ok(())
    }
}

fn references(value: &Value, output: &mut Vec<ObjectRef>) -> Result<(), WorkflowError> {
    match value {
        Value::Object(fields)
            if fields.contains_key("path")
                && fields.contains_key("sha256")
                && fields.contains_key("bytes") =>
        {
            output.push(serde_json::from_value(value.clone()).map_err(|error| {
                WorkflowError::internal(format!("Invalid saved object: {error}"))
            })?);
        }
        Value::Object(fields) => {
            for value in fields.values() {
                references(value, output)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                references(value, output)?;
            }
        }
        _ => {}
    }
    Ok(())
}
