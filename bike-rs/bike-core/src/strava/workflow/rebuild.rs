use crate::{jobs::JobQueue, workflow_error::WorkflowError};
use chrono::NaiveDate;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

/// The changed imports, segments and date range handed from apply to rebuild.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Rebuild {
    pub import_ids: Vec<i32>,
    pub segment_ids: Vec<i32>,
    pub dirty_from: Option<NaiveDate>,
}

impl Rebuild {
    pub(crate) fn import(id: i32, segment_ids: Vec<i32>, dirty_from: NaiveDate) -> Self {
        Self {
            import_ids: vec![id],
            segment_ids,
            dirty_from: Some(dirty_from),
        }
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.import_ids.extend(other.import_ids);
        self.import_ids.sort_unstable();
        self.import_ids.dedup();
        self.segment_ids.extend(other.segment_ids);
        self.segment_ids.sort_unstable();
        self.segment_ids.dedup();
        self.dirty_from = self.dirty_from.into_iter().chain(other.dirty_from).min();
    }

    pub(crate) async fn run(
        &self,
        db: &DatabaseConnection,
        user_id: i32,
    ) -> Result<serde_json::Value, WorkflowError> {
        if self.import_ids.is_empty() && self.segment_ids.is_empty() && self.dirty_from.is_none() {
            return Ok(serde_json::json!({"outcome":"unchanged"}));
        }
        let tasks = JobQueue::new(db.clone());
        crate::activity_import_lifecycle::complete_activity_imports(
            db,
            &tasks,
            user_id,
            &self.import_ids,
            self.segment_ids.clone(),
            self.dirty_from,
        )
        .await?;
        tasks
            .rebuild_segment_analytics(self.segment_ids.clone())
            .await?;
        Ok(
            serde_json::json!({"outcome":"queued","import_ids":self.import_ids,"segment_ids":self.segment_ids,"dirty_from":self.dirty_from}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collection_rebuild_combines_changes_once_from_the_earliest_day() {
        let first = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let later = NaiveDate::from_ymd_opt(2026, 10, 12).unwrap();
        let mut rebuild = Rebuild::default();
        rebuild.merge(Rebuild::import(2, vec![3, 1], later));
        rebuild.merge(Rebuild::import(2, vec![1, 4], first));
        assert_eq!(rebuild.import_ids, vec![2]);
        assert_eq!(rebuild.segment_ids, vec![1, 3, 4]);
        assert_eq!(rebuild.dirty_from, Some(first));
    }
}
