use crate::activity_achievements::ActivityAchievementHighlight;
use sea_orm::entity::prelude::*;
use sea_orm::{JoinType, QueryOrder, QuerySelect, Set};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, ToSchema)]
#[schema(as = ActivityEarnedAward)]
#[sea_orm(table_name = "activity_earned_awards")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub position: i32,
    pub display_position: i32,
    pub segment_id: i32,
    pub segment_title: String,
    pub effort_index: i32,
    pub overall_rank: Option<i32>,
    pub personal_rank: Option<i32>,
    pub personal_best_duration_seconds: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::activity_award_outputs::Entity",
        from = "Column::ActivityId",
        to = "super::activity_award_outputs::Column::ActivityId"
    )]
    Output,
}
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub async fn for_user_activity_ids(
        db: &impl ConnectionTrait,
        user_id: i32,
        activity_ids: &[i32],
    ) -> Result<Vec<Self>, DbErr> {
        if activity_ids.is_empty() {
            return Ok(Vec::new());
        }
        Entity::find()
            .join(JoinType::InnerJoin, Relation::Output.def())
            .filter(super::activity_award_outputs::Column::UserId.eq(user_id))
            .filter(Column::ActivityId.is_in(activity_ids.iter().copied()))
            .order_by_asc(Column::ActivityId)
            .order_by_asc(Column::DisplayPosition)
            .all(db)
            .await
    }

    pub(crate) async fn for_activities(db: &impl ConnectionTrait, activity_ids: &[i32]) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::ActivityId.is_in(activity_ids.iter().copied()))
            .order_by_asc(Column::ActivityId)
            .order_by_asc(Column::Position)
            .all(db)
            .await
    }

    pub(crate) async fn record(
        db: &impl ConnectionTrait,
        activity_id: i32,
        highlights: Vec<ActivityAchievementHighlight>,
    ) -> Result<(), DbErr> {
        if highlights.is_empty() {
            return Ok(());
        }
        let mut highlights: Vec<_> = highlights.into_iter().enumerate().collect();
        highlights.sort_by_key(|(recording_index, highlight)| (highlight.display_order(), *recording_index));
        let rows = highlights
            .into_iter()
            .enumerate()
            .map(|(display_index, (recording_index, highlight))| {
                ActiveModel::from_highlight(
                    activity_id,
                    (recording_index + 1) as i32,
                    (display_index + 1) as i32,
                    highlight,
                )
            });
        Entity::insert_many(rows).exec_without_returning(db).await?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn into_highlight(self) -> ActivityAchievementHighlight {
        ActivityAchievementHighlight {
            segment_id: self.segment_id,
            segment_title: self.segment_title,
            effort_index: self.effort_index,
            overall_rank: self.overall_rank,
            personal_rank: self.personal_rank,
            personal_best_duration_seconds: self.personal_best_duration_seconds,
        }
    }
}

impl ActiveModel {
    fn from_highlight(
        activity_id: i32,
        position: i32,
        display_position: i32,
        highlight: ActivityAchievementHighlight,
    ) -> Self {
        Self {
            activity_id: Set(activity_id),
            position: Set(position),
            display_position: Set(display_position),
            segment_id: Set(highlight.segment_id),
            segment_title: Set(highlight.segment_title),
            effort_index: Set(highlight.effort_index),
            overall_rank: Set(highlight.overall_rank),
            personal_rank: Set(highlight.personal_rank),
            personal_best_duration_seconds: Set(highlight.personal_best_duration_seconds),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support as fixtures;

    #[tokio::test]
    async fn worker_saves_display_positions_using_recording_order_for_ties() {
        let base = fixtures::activity_earned_awards().into_highlight();
        let inputs = [
            (None, Some(1), "Zulu"),
            (Some(1), Some(1), "Beta"),
            (Some(7), Some(1), "Any"),
            (Some(1), Some(1), "Alpha"),
            (Some(2), Some(1), "Any"),
            (None, Some(2), "Any"),
            (None, Some(3), "Any"),
            (Some(1), Some(1), "Alpha"),
            (None, Some(1), "Alpha"),
        ];
        let highlights = inputs
            .into_iter()
            .enumerate()
            .map(
                |(index, (overall_rank, personal_rank, title))| ActivityAchievementHighlight {
                    overall_rank,
                    personal_rank,
                    segment_title: title.into(),
                    effort_index: (index + 1) as i32,
                    ..base.clone()
                },
            )
            .collect();
        assert_recorded_order(highlights, &[2, 4, 8, 5, 3, 1, 9, 6, 7]).await;
    }

    #[tokio::test]
    async fn worker_saves_numeric_top_ten_order_in_both_input_orders() {
        let base = fixtures::activity_earned_awards().into_highlight();
        for (ranks, expected) in [([10, 2], [2, 1]), ([2, 10], [1, 2])] {
            let first = ActivityAchievementHighlight {
                segment_id: 12,
                segment_title: "Zulu".into(),
                effort_index: 5,
                overall_rank: Some(ranks[0]),
                personal_best_duration_seconds: Some(95),
                ..base.clone()
            };
            let second = ActivityAchievementHighlight {
                segment_id: 11,
                segment_title: "Alpha".into(),
                effort_index: 2,
                overall_rank: Some(ranks[1]),
                ..base.clone()
            };
            assert_recorded_order(vec![first, second], &expected).await;
        }
    }

    async fn assert_recorded_order(highlights: Vec<ActivityAchievementHighlight>, expected_positions: &[i32]) {
        let db = fixtures::postgres_mock()
            .append_exec_results([fixtures::written()])
            .into_connection();
        Model::record(&db, 42, highlights.clone()).await.unwrap();
        let log = db.into_transaction_log();
        let statement = &log[0].statements()[0];
        let expected: Vec<sea_orm::Value> = expected_positions
            .iter()
            .enumerate()
            .flat_map(|(display_index, &position)| {
                let highlight = &highlights[(position - 1) as usize];
                [
                    42.into(),
                    position.into(),
                    ((display_index + 1) as i32).into(),
                    highlight.segment_id.into(),
                    highlight.segment_title.clone().into(),
                    highlight.effort_index.into(),
                    highlight.overall_rank.into(),
                    highlight.personal_rank.into(),
                    highlight.personal_best_duration_seconds.into(),
                ]
            })
            .collect();
        assert_eq!(statement.values.as_ref().unwrap().0, expected, "input: {highlights:?}");
    }
}
