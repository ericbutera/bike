use super::*;
use crate::entities::activity_event_history;
use sea_orm::TransactionTrait;

#[derive(Debug, Default)]
pub struct ActivityDetailsUpdate {
    pub title: Option<String>,
    pub activity_type: Option<ActivityType>,
    pub sport: Option<ActivitySport>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub private_notes: Option<String>,
    pub gear_name: Option<String>,
}

#[derive(Debug)]
pub struct ActivityEditOutcome {
    pub activity: Model,
    pub sport_changed: bool,
    pub affects_cycling: bool,
}

impl ActivityDetailsUpdate {
    fn apply(self, before: &Model) -> ActiveModel {
        let mut active: ActiveModel = before.clone().into();
        if let Some(title) = self.title.filter(|title| title != &before.title) {
            active.title = Set(title);
        }
        if let Some(kind) = self.activity_type.filter(|kind| kind.as_str() != before.activity_type) {
            active.activity_type = Set(kind.as_str().to_string());
        }
        if let Some(sport) = self.sport.filter(|sport| sport.as_str() != before.sport) {
            active.sport = Set(sport.as_str().to_string());
            if !sport.is_cycling() {
                active.estimated_ftp_watts = Set(None);
            }
        }
        let stored = before.derived_data_json.clone().unwrap_or_default();
        let original = stored.source_metadata().cloned().unwrap_or_default();
        let mut metadata = original.clone();
        if let Some(location) = self.location {
            metadata.location = optional_text(location);
        }
        if let Some(description) = self.description {
            metadata.description = optional_text(description);
        }
        if let Some(notes) = self.private_notes {
            metadata.private_notes = optional_text(notes);
        }
        if let Some(name) = self.gear_name {
            if let Some(gear) = metadata.gear.as_mut() {
                gear.name = optional_text(name);
            }
        }
        if metadata != original {
            active.derived_data_json = Set(Some(stored.with_source_metadata(metadata)));
        }
        active
    }
}

fn optional_text(value: String) -> Option<String> {
    let text = value.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

impl Model {
    pub async fn update_details(
        self,
        db: &DatabaseConnection,
        mut update: ActivityDetailsUpdate,
    ) -> Result<ActivityEditOutcome, WorkflowError> {
        update.title = update.title.map(|title| Self::normalize_title(&title)).transpose()?;
        let transaction = db.begin().await?;
        let before = Self::lock_owned(&transaction, self.id, self.user_id)
            .await?
            .ok_or_else(|| WorkflowError::not_found("Activity not found"))?;
        let sport_changed = update.sport.is_some_and(|sport| sport.as_str() != before.sport);
        let location = update.location.clone().map(optional_text);
        let active = update.apply(&before);
        if !active.is_changed() {
            transaction.commit().await?;
            return Ok(ActivityEditOutcome {
                activity: before,
                sport_changed: false,
                affects_cycling: false,
            });
        }
        let after = active.update(&transaction).await?;
        crate::entities::activity_metadata::Model::sync_edit(&transaction, &before, &after).await?;
        if let Some(location) = location {
            crate::entities::activity_cards::Model::set_location(&transaction, after.id, location).await?;
        }
        let cycling = before.is_bike_activity() || after.is_bike_activity();
        let affects_cycling = sport_changed && cycling;
        if sport_changed {
            Self::refresh_sport_eligibility(&transaction, &after, affects_cycling).await?;
        }
        activity_event_history::Model::append(&transaction, activity_event_history::Event::manual(&before, &after)?)
            .await?;
        transaction.commit().await?;
        Ok(ActivityEditOutcome {
            activity: after,
            sport_changed,
            affects_cycling,
        })
    }

    async fn refresh_sport_eligibility(
        db: &impl ConnectionTrait,
        activity: &Self,
        affects_cycling: bool,
    ) -> Result<(), WorkflowError> {
        use crate::entities::{activity_analytics, activity_training_analyses, analytics_user_states, segment_efforts};
        if !activity.is_bike_activity() {
            segment_efforts::Entity::delete_for_owned_activity(db, activity.user_id, activity.id).await?;
            activity_analytics::Entity::delete_by_activity_id(db, activity.id).await?;
            activity_training_analyses::Entity::delete_by_activity_id(db, activity.id).await?;
        }
        if affects_cycling {
            analytics_user_states::Model::mark_dirty(
                db,
                activity.user_id,
                activity.started_at.date_naive(),
                activity.updated_at,
            )
            .await?;
        }
        Ok(())
    }
}
