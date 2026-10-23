use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};

const STORAGE_FORMAT_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityAchievementHighlight {
    pub segment_id: i32,
    pub segment_title: String,
    pub effort_index: i32,
    pub overall_rank: Option<i32>,
    pub personal_rank: Option<i32>,
    pub personal_best_duration_seconds: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivityAchievementKind {
    Kom,
    Top10,
    Pr,
    PersonalPodium,
}

pub(crate) fn activity_achievement_kind(
    overall_rank: Option<i32>,
    personal_rank: Option<i32>,
) -> Option<ActivityAchievementKind> {
    if overall_rank == Some(1) {
        return Some(ActivityAchievementKind::Kom);
    }

    if overall_rank.is_some_and(|rank| (2..=10).contains(&rank)) {
        return Some(ActivityAchievementKind::Top10);
    }

    if personal_rank == Some(1) {
        return Some(ActivityAchievementKind::Pr);
    }

    if personal_rank.is_some_and(|rank| (2..=3).contains(&rank)) {
        return Some(ActivityAchievementKind::PersonalPodium);
    }

    None
}

impl ActivityAchievementHighlight {
    pub(crate) fn display_order(&self) -> (i32, i32) {
        match activity_achievement_kind(self.overall_rank, self.personal_rank) {
            Some(ActivityAchievementKind::Kom) => (0, 0),
            Some(ActivityAchievementKind::Top10) => (1, self.overall_rank.unwrap_or_default()),
            Some(ActivityAchievementKind::Pr) => (2, 1),
            Some(ActivityAchievementKind::PersonalPodium) => (2, self.personal_rank.unwrap_or_default()),
            None => (3, 0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct StoredActivityAchievementHighlights {
    #[serde(default = "storage_format_version")]
    pub v: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ActivityAchievementHighlight>,
}

impl Default for StoredActivityAchievementHighlights {
    fn default() -> Self {
        Self {
            v: STORAGE_FORMAT_VERSION,
            items: Vec::new(),
        }
    }
}

fn storage_format_version() -> u8 {
    STORAGE_FORMAT_VERSION
}

impl StoredActivityAchievementHighlights {
    pub fn from_items(items: Vec<ActivityAchievementHighlight>) -> Self {
        Self {
            v: STORAGE_FORMAT_VERSION,
            items,
        }
    }
}

#[cfg(test)]
#[path = "activity_achievements_tests.rs"]
mod tests;

/// Convert the retained legacy decision, or save the first decision from live results.
pub async fn record(
    db: &(impl sea_orm::ConnectionTrait + sea_orm::TransactionTrait),
    activity: &crate::entities::activities::Model,
) -> Result<(), sea_orm::DbErr> {
    use crate::entities::{activities, activity_analytics, activity_award_outputs};
    use sea_orm::TransactionSession;

    let txn = db.begin().await?;
    activities::Model::lock_for_analytics(&txn, &[activity.id]).await?;
    if activity_award_outputs::Model::for_activity(&txn, activity.id)
        .await?
        .is_none()
    {
        let legacy = activity_analytics::Model::list_by_user_activity_ids(&txn, activity.user_id, &[activity.id])
            .await?
            .into_iter()
            .find_map(|row| row.achievement_highlights_json);
        let highlights = match legacy.as_ref() {
            Some(stored) => stored.items.clone(),
            None => first_live_decision(&txn, activity).await?,
        };
        activity_award_outputs::Model::record(&txn, activity.id, activity.user_id, highlights).await?;
        if legacy.is_some() {
            activity_analytics::Model::clear_legacy_highlights(&txn, activity.id, activity.user_id).await?;
        }
    }
    txn.commit().await
}

async fn first_live_decision(
    db: &impl sea_orm::ConnectionTrait,
    activity: &crate::entities::activities::Model,
) -> Result<Vec<ActivityAchievementHighlight>, sea_orm::DbErr> {
    let efforts =
        crate::entities::segment_efforts::Model::list_by_user_activity_ids(db, activity.user_id, &[activity.id])
            .await?;
    let results = LiveResults::load(db, activity.user_id, &efforts).await?;
    Ok(results.highlights(&efforts))
}

#[derive(Default)]
struct LiveResults {
    titles: std::collections::HashMap<i32, String>,
    overall: std::collections::HashMap<i32, i32>,
    personal: std::collections::HashMap<i32, i32>,
    personal_bests: std::collections::HashMap<i32, i32>,
}

impl LiveResults {
    async fn load(
        db: &impl sea_orm::ConnectionTrait,
        user_id: i32,
        efforts: &[crate::entities::segment_efforts::Model],
    ) -> Result<Self, sea_orm::DbErr> {
        use crate::entities::{segment_overall_achievements, segment_personal_achievements, segments};
        let segment_ids: Vec<_> = efforts
            .iter()
            .map(|effort| effort.segment_id)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut results = Self::default();
        if segment_ids.is_empty() {
            return Ok(results);
        }
        results.titles = segments::Model::titles_by_ids(db, &segment_ids)
            .await?
            .into_iter()
            .map(|segment| (segment.id, segment.title))
            .collect();
        for row in segment_overall_achievements::Model::for_user_segments(db, user_id, &segment_ids).await? {
            results.overall.insert(row.effort_id, row.position);
        }
        for row in segment_personal_achievements::Model::for_user_segments(db, user_id, &segment_ids).await? {
            results.personal.insert(row.effort_id, row.position);
            if row.position == 1 {
                results.personal_bests.insert(row.segment_id, row.duration_seconds);
            }
        }
        Ok(results)
    }

    fn highlights(&self, efforts: &[crate::entities::segment_efforts::Model]) -> Vec<ActivityAchievementHighlight> {
        efforts.iter().filter_map(|effort| self.highlight(effort)).collect()
    }

    fn highlight(&self, effort: &crate::entities::segment_efforts::Model) -> Option<ActivityAchievementHighlight> {
        let overall_rank = self.overall.get(&effort.id).copied();
        let personal_rank = self.personal.get(&effort.id).copied();
        if overall_rank.is_none() && personal_rank.is_none() {
            return None;
        }
        Some(ActivityAchievementHighlight {
            segment_id: effort.segment_id,
            segment_title: self
                .titles
                .get(&effort.segment_id)
                .cloned()
                .unwrap_or_else(|| format!("Segment {}", effort.segment_id)),
            effort_index: effort.effort_index,
            overall_rank,
            personal_rank,
            personal_best_duration_seconds: self.personal_bests.get(&effort.segment_id).copied(),
        })
    }
}
