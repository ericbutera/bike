use crate::activity_achievements::ActivityAchievementHighlight;
use crate::bike_tasks::{ActivityTask, Priority};
use sea_orm::entity::prelude::*;
use sea_orm::{FromQueryResult, Set, Statement};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_award_outputs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i32,
    pub user_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

#[derive(FromQueryResult)]
struct PendingActivity {
    activity_id: i32,
    user_id: i32,
}

impl Model {
    pub async fn pending_tasks(db: &impl ConnectionTrait, after_id: i32) -> Result<Vec<ActivityTask>, DbErr> {
        let rows = PendingActivity::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT a.id AS activity_id, a.user_id FROM activities a
             WHERE a.id > $1 AND NOT EXISTS
                 (SELECT 1 FROM activity_award_outputs o WHERE o.activity_id = a.id)
             ORDER BY a.id LIMIT 16",
            [after_id.into()],
        ))
        .all(db)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| ActivityTask {
                activity_id: row.activity_id,
                user_id: row.user_id,
                priority: Priority::Background,
            })
            .collect())
    }

    pub(crate) async fn for_activity(db: &impl ConnectionTrait, activity_id: i32) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(activity_id).one(db).await
    }

    pub(crate) async fn for_activities(db: &impl ConnectionTrait, activity_ids: &[i32]) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::ActivityId.is_in(activity_ids.iter().copied()))
            .all(db)
            .await
    }

    /// The caller saves the output and its awards in the same transaction.
    pub(crate) async fn record(
        db: &impl ConnectionTrait,
        activity_id: i32,
        user_id: i32,
        highlights: Vec<ActivityAchievementHighlight>,
    ) -> Result<(), DbErr> {
        Entity::insert(ActiveModel {
            activity_id: Set(activity_id),
            user_id: Set(user_id),
        })
        .exec_without_returning(db)
        .await?;
        super::activity_earned_awards::Model::record(db, activity_id, highlights).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support as fixtures;
    use std::collections::BTreeMap;

    #[tokio::test]
    async fn backfill_selects_missing_decisions_in_bounded_activity_order_with_their_owners() {
        let rows: Vec<BTreeMap<String, sea_orm::Value>> = [(42, 7), (77, 8)]
            .into_iter()
            .map(|(activity_id, user_id)| {
                [
                    ("activity_id".into(), activity_id.into()),
                    ("user_id".into(), user_id.into()),
                ]
                .into()
            })
            .collect();
        let db = fixtures::postgres_mock().append_query_results([rows]).into_connection();
        let tasks = Model::pending_tasks(&db, 30).await.unwrap();
        assert_eq!(
            tasks
                .iter()
                .map(|t| (t.activity_id, t.user_id, t.priority))
                .collect::<Vec<_>>(),
            vec![(42, 7, Priority::Background), (77, 8, Priority::Background)]
        );
        let log = db.into_transaction_log();
        let query = &log[0].statements()[0];
        assert!(query.sql.contains("NOT EXISTS"));
        assert!(query.sql.contains("ORDER BY a.id LIMIT 16"));
        assert_eq!(query.values.as_ref().unwrap().0, vec![30.into()]);
    }
}
