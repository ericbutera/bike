//! Current all-time global standings, with one position per athlete.
use crate::entities::{segment_efforts, segment_overall_achievements as achievements, segments};
use sea_orm::{ConnectionTrait, DbErr, TransactionSession, TransactionTrait};
use std::collections::HashSet;

fn overall_top_ten(mut efforts: Vec<segment_efforts::Model>) -> Vec<achievements::Model> {
    efforts.sort_by_key(|effort| (effort.duration_seconds, effort.id));
    let mut athletes = HashSet::new();
    efforts
        .into_iter()
        .filter(|effort| athletes.insert(effort.user_id))
        .take(10)
        .zip(1..=10)
        .map(|(effort, position)| achievements::Model {
            segment_id: effort.segment_id,
            position,
            user_id: effort.user_id,
            effort_id: effort.id,
            duration_seconds: effort.duration_seconds,
        })
        .collect()
}

/// Analytics and the rollout backfill call the same saved-effort operation.
#[tracing::instrument(name = "achievements.overall.rebuild", skip_all, fields(segment_id))]
pub async fn rebuild(db: &(impl ConnectionTrait + TransactionTrait), segment_id: i32) -> Result<(), DbErr> {
    let txn = db.begin().await?;
    let Some(segment) = segments::Model::lock_for_analytics(&txn, segment_id).await? else {
        return Ok(());
    };
    let efforts = segment_efforts::Model::list_for_segment_analytics(&txn, &[segment.id]).await?;
    achievements::Model::replace_for_segment(&txn, segment.id, overall_top_ten(efforts)).await?;
    txn.commit().await
}

#[cfg(test)]
mod tests;
