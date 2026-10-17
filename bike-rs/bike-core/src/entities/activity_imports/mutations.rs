use super::*;
use crate::activity_import_lifecycle::*;
use sea_orm::ActiveModelTrait;

impl Model {
    pub async fn mark_processing(
        &self,
        db: &impl ConnectionTrait,
        stage: &str,
        activity_id: Option<i32>,
        now: DateTime<Utc>,
    ) -> Result<Self, DbErr> {
        let mut active = ActiveModel {
            last_processing_event_at: Set(Some(now)),
            ..ActiveModel::from(self.clone()).with_stage(ACTIVITY_IMPORT_STATUS_PROCESSING, stage)
        };
        if let Some(activity_id) = activity_id {
            active.activity_id = Set(Some(activity_id));
        }
        active.update(db).await
    }

    pub async fn mark_processed(self, db: &impl ConnectionTrait) -> Result<Self, DbErr> {
        let completed = ActiveModel::from(self).with_completion(ACTIVITY_IMPORT_STATUS_PROCESSED);
        ActiveModel {
            last_processing_event_at: Set(Some(Utc::now())),
            ..completed
        }
        .update(db)
        .await
    }

    pub async fn mark_failed(self, db: &impl ConnectionTrait, stage: &str, error_message: &str) -> Result<Self, DbErr> {
        let attempts = self.processing_attempts.saturating_add(1);
        ActiveModel {
            processing_error: Set(Some(error_message.into())),
            processing_attempts: Set(attempts),
            last_processing_event_at: Set(Some(Utc::now())),
            ..ActiveModel::from(self).with_stage(ACTIVITY_IMPORT_STATUS_FAILED, stage)
        }
        .update(db)
        .await
    }

    pub async fn mark_duplicate(&self, db: &impl ConnectionTrait, duplicate_activity_id: i32) -> Result<Self, DbErr> {
        let completed = ActiveModel::from(self.clone()).with_completion(ACTIVITY_IMPORT_STATUS_DUPLICATE);
        ActiveModel {
            activity_id: Set(Some(duplicate_activity_id)),
            last_processing_event_at: Set(Some(Utc::now())),
            ..completed
        }
        .update(db)
        .await
    }
}

impl Model {
    pub async fn store_upload(
        db: &impl ConnectionTrait,
        user_id: i32,
        source: &str,
        upload: &crate::activity_import_pipeline::ActivityUploadPayload,
        relative_path: String,
    ) -> Result<Self, DbErr> {
        ActiveModel {
            user_id: Set(user_id),
            source: Set(source.to_string()),
            format: Set(upload.format.clone()),
            status: Set(ACTIVITY_IMPORT_STATUS_PROCESSING.into()),
            processing_stage: Set(ACTIVITY_IMPORT_STAGE_RAW_STORED.into()),
            last_processing_event_at: Set(Some(Utc::now())),
            original_filename: Set(upload.original_filename.clone()),
            storage_path: Set(relative_path),
            size_bytes: Set(upload.bytes.len() as i64),
            mime_type: Set(upload.mime_type.clone()),
            ..Default::default()
        }
        .insert(db)
        .await
    }
}

impl Model {
    pub async fn mark_native_processed(self, db: &impl ConnectionTrait) -> Result<Self, DbErr> {
        ActiveModel::from(self)
            .with_completion(ACTIVITY_IMPORT_STATUS_PROCESSED)
            .update(db)
            .await
    }
}

impl ActiveModel {
    fn with_stage(self, status: &str, stage: &str) -> Self {
        Self {
            status: Set(status.into()),
            processing_stage: Set(stage.into()),
            processing_error: Set(None),
            ..self
        }
    }

    fn with_completion(self, status: &str) -> Self {
        Self {
            processed_at: Set(Some(Utc::now())),
            ..self.with_stage(status, ACTIVITY_IMPORT_STAGE_COMPLETE)
        }
    }
}
