use super::{ActiveModel, Column, Entity, Model};
use crate::segment_support::MatchedSegmentEffort;
use sea_orm::sea_query::{CaseStatement, Expr, SimpleExpr};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set};
use std::collections::{BTreeMap, HashSet};

type MatchedSegments = Vec<(i32, Vec<MatchedSegmentEffort>)>;

impl Entity {
    /// The caller retains its activity lock and transaction across the complete match set.
    pub(crate) async fn reconcile_activity_matches(
        db: &impl ConnectionTrait,
        user_id: i32,
        activity_id: i32,
        matches: MatchedSegments,
    ) -> Result<(), DbErr> {
        let existing = Model::list_by_user_activity_ids(db, user_id, &[activity_id]).await?;
        let mut segment_ids: Vec<_> = existing.iter().map(|effort| effort.segment_id).collect();
        segment_ids.extend(matches.iter().map(|(segment_id, _)| *segment_id));
        segment_ids.sort_unstable();
        segment_ids.dedup();
        super::super::segments::Model::lock_matching_scope(db, &segment_ids).await?;
        let plan = Reconciliation::for_activity(user_id, activity_id, existing, matches);
        plan.persist(db, user_id, activity_id).await
    }

    /// Reuse traversal identity rules without removing another segment's efforts.
    pub(crate) async fn reconcile_pair_matches(
        db: &impl ConnectionTrait,
        user_id: i32,
        activity_id: i32,
        segment_id: i32,
        matches: Vec<MatchedSegmentEffort>,
    ) -> Result<bool, DbErr> {
        let existing = Model::list_for_owned_pair(db, user_id, activity_id, segment_id).await?;
        let plan = Reconciliation::for_activity(user_id, activity_id, existing, vec![(segment_id, matches)]);
        let changed = !plan.new.is_empty() || !plan.changed.is_empty();
        let changed = changed || !plan.removed.is_empty();
        plan.persist(db, user_id, activity_id).await?;
        Ok(changed)
    }
}

struct Reconciliation {
    new: Vec<ActiveModel>,
    changed: Vec<ActiveModel>,
    removed: Vec<i32>,
}

impl Reconciliation {
    fn for_activity(user_id: i32, activity_id: i32, existing: Vec<Model>, matches: MatchedSegments) -> Self {
        let mut saved = BTreeMap::new();
        let old_ids: Vec<_> = existing.iter().map(|effort| effort.id).collect();
        for effort in existing {
            saved
                .entry((
                    effort.segment_id,
                    effort.start_route_point_index,
                    effort.end_route_point_index,
                ))
                .or_insert(effort);
        }
        let mut retained = HashSet::new();
        let mut plan = Self {
            new: Vec::new(),
            changed: Vec::new(),
            removed: Vec::new(),
        };
        for (segment_id, matches) in matches {
            for (index, matched) in matches.into_iter().enumerate() {
                let attempt = (index + 1) as i32;
                let key = (
                    segment_id,
                    matched.start_route_point_index,
                    matched.end_route_point_index,
                );
                let previous = saved.remove(&key);
                if let Some(previous) = previous {
                    retained.insert(previous.id);
                    if !previous.matches_recording(&matched, attempt) {
                        let mut changed = matched.into_effort(user_id, activity_id, segment_id, attempt);
                        changed.id = Set(previous.id);
                        plan.changed.push(changed);
                    }
                } else {
                    plan.new
                        .push(matched.into_effort(user_id, activity_id, segment_id, attempt));
                }
            }
        }
        plan.removed = old_ids.into_iter().filter(|id| !retained.contains(id)).collect();
        plan
    }

    async fn persist(self, db: &impl ConnectionTrait, user_id: i32, activity_id: i32) -> Result<(), DbErr> {
        for ids in self.removed.chunks(500) {
            Entity::delete_many()
                .filter(Column::UserId.eq(user_id))
                .filter(Column::ActivityId.eq(activity_id))
                .filter(Column::Id.is_in(ids.iter().copied()))
                .exec(db)
                .await?;
        }
        for batch in self.changed.chunks(500) {
            let mut update = Entity::update_many();
            for column in [
                Column::EffortIndex,
                Column::StartElapsedSeconds,
                Column::EndElapsedSeconds,
                Column::DurationSeconds,
                Column::DistanceMeters,
            ] {
                update = update.col_expr(column, changed_values(batch, column));
            }
            let result = update
                .col_expr(Column::OverallRank, Expr::value(None::<i32>))
                .col_expr(Column::UserRank, Expr::value(None::<i32>))
                .col_expr(Column::UpdatedAt, Expr::current_timestamp())
                .filter(Column::UserId.eq(user_id))
                .filter(Column::ActivityId.eq(activity_id))
                .filter(Column::Id.is_in(batch.iter().map(|row| row.id.clone().unwrap())))
                .exec(db)
                .await?;
            if result.rows_affected != batch.len() as u64 {
                return Err(DbErr::RecordNotFound(
                    "segment effort changed during activity reconciliation".into(),
                ));
            }
        }
        for batch in self.new.chunks(500) {
            Entity::insert_many(batch.to_vec()).exec_without_returning(db).await?;
        }
        Ok(())
    }
}

fn changed_values(rows: &[ActiveModel], column: Column) -> SimpleExpr {
    let mut cases = CaseStatement::new();
    for row in rows {
        cases = cases.case(Column::Id.eq(row.id.clone().unwrap()), row.get(column).unwrap());
    }
    cases.finally(Expr::col(column)).into()
}

impl Model {
    fn matches_recording(&self, matched: &MatchedSegmentEffort, attempt: i32) -> bool {
        let recording = MatchedSegmentEffort {
            start_route_point_index: self.start_route_point_index,
            end_route_point_index: self.end_route_point_index,
            start_elapsed_seconds: self.start_elapsed_seconds,
            end_elapsed_seconds: self.end_elapsed_seconds,
            duration_seconds: self.duration_seconds,
            distance_meters: self.distance_meters,
        };
        self.effort_index == attempt && recording == *matched
    }
}

impl MatchedSegmentEffort {
    pub(super) fn into_effort(self, user_id: i32, activity_id: i32, segment_id: i32, attempt: i32) -> ActiveModel {
        ActiveModel {
            user_id: Set(user_id),
            activity_id: Set(activity_id),
            segment_id: Set(segment_id),
            effort_index: Set(attempt),
            start_route_point_index: Set(self.start_route_point_index),
            end_route_point_index: Set(self.end_route_point_index),
            start_elapsed_seconds: Set(self.start_elapsed_seconds),
            end_elapsed_seconds: Set(self.end_elapsed_seconds),
            duration_seconds: Set(self.duration_seconds),
            distance_meters: Set(self.distance_meters),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests;
