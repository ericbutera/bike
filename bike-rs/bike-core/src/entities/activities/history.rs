use super::*;
use crate::entities::activity_event_history;
use sea_orm::TransactionTrait;

impl Model {
    pub async fn update_details(
        self,
        db: &DatabaseConnection,
        title: Option<String>,
        activity_type: Option<ActivityType>,
    ) -> Result<Self, WorkflowError> {
        let title = title.map(|title| Self::normalize_title(&title)).transpose()?;
        let transaction = db.begin().await?;
        let before = Self::lock_owned(&transaction, self.id, self.user_id)
            .await?
            .ok_or_else(|| WorkflowError::not_found("Activity not found"))?;
        let title_changed = title.as_ref().is_some_and(|title| title != &before.title);
        let type_changed = activity_type.is_some_and(|kind| kind.as_str() != before.activity_type);
        if !title_changed && !type_changed {
            transaction.commit().await?;
            return Ok(before);
        }
        let mut active: ActiveModel = before.clone().into();
        if let Some(title) = title {
            active.title = Set(title);
        }
        if let Some(activity_type) = activity_type {
            active.activity_type = Set(activity_type.as_str().to_string());
        }
        let after = active.update(&transaction).await?;
        activity_event_history::Model::append(&transaction, activity_event_history::Event::manual(&before, &after)?)
            .await?;
        transaction.commit().await?;
        Ok(after)
    }
}
