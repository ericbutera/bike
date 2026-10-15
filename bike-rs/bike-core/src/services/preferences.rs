use crate::entities::user_preferences::{GoalBackfillChange, Model, PreferencesUpdate};
use crate::jobs::JobQueue;
use crate::workflow_error::WorkflowError;
use crate::xc_goal_backfill::{clear_user_xc_goal_backfill_state, queue_user_xc_goal_backfill};
use sea_orm::DatabaseConnection;

pub async fn update_preferences(
    db: &DatabaseConnection,
    tasks: &JobQueue,
    user_id: i32,
    update: PreferencesUpdate,
) -> Result<Model, WorkflowError> {
    let saved = Model::save_preferences(db, user_id, update).await?;
    match saved.goal_backfill {
        GoalBackfillChange::Queue => {
            queue_user_xc_goal_backfill(db, tasks, user_id).await?;
        }
        GoalBackfillChange::Clear => {
            clear_user_xc_goal_backfill_state(db, user_id).await?;
        }
        GoalBackfillChange::Unchanged => {}
    }
    Ok(Model::for_user(db, user_id).await?.unwrap_or(saved.model))
}
