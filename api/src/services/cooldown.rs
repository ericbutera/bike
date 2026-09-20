use crate::app_error::AppError;
use kaleido::glass::cooldown::{
    BackoffStrategy as CooldownBackoffStrategy, CooldownService as SharedCooldownService,
};
use sea_orm::DatabaseConnection;

#[derive(Debug, Clone, Copy)]
pub enum CooldownType {
    ReportGeneration,
}

impl CooldownType {
    fn action(&self) -> &'static str {
        match self {
            CooldownType::ReportGeneration => "training_report_generation",
        }
    }

    fn strategy(&self) -> CooldownBackoffStrategy {
        match self {
            CooldownType::ReportGeneration => CooldownBackoffStrategy::Simple,
        }
    }

    fn duration_seconds(&self) -> i64 {
        match self {
            CooldownType::ReportGeneration => 30,
        }
    }

    fn message(&self, retry_after: i64) -> String {
        match self {
            CooldownType::ReportGeneration => {
                if retry_after > 0 {
                    format!(
                        "A report is already generating. Stay on that report until it finishes before starting another one. Try again in {} seconds.",
                        retry_after
                    )
                } else {
                    "A report is already generating. Stay on that report until it finishes before starting another one."
                        .to_owned()
                }
            }
        }
    }
}

pub struct CooldownService;

impl CooldownService {
    fn subject_type(subject_id: Option<i32>) -> &'static str {
        if subject_id.is_some() {
            "user"
        } else {
            "global"
        }
    }

    pub async fn acquire(
        db: &DatabaseConnection,
        cooldown_type: CooldownType,
        subject_id: i32,
    ) -> Result<CooldownLease, AppError> {
        SharedCooldownService::check_and_update(
            db,
            Self::subject_type(Some(subject_id)),
            Some(subject_id),
            cooldown_type.action(),
            cooldown_type.strategy(),
            cooldown_type.duration_seconds(),
            |retry_after| cooldown_type.message(retry_after),
        )
        .await
        .map_err(AppError::from)?;

        Ok(CooldownLease {
            db: db.clone(),
            cooldown_type,
            subject_id,
            released: false,
        })
    }
}

pub struct CooldownLease {
    db: DatabaseConnection,
    cooldown_type: CooldownType,
    subject_id: i32,
    released: bool,
}

impl CooldownLease {
    pub async fn release(&mut self) -> Result<(), AppError> {
        SharedCooldownService::reset(
            &self.db,
            CooldownService::subject_type(Some(self.subject_id)),
            Some(self.subject_id),
            self.cooldown_type.action(),
        )
        .await
        .map_err(AppError::from)?;
        self.released = true;
        Ok(())
    }
}

impl Drop for CooldownLease {
    fn drop(&mut self) {
        if self.released {
            return;
        }

        let db = self.db.clone();
        let subject_id = self.subject_id;
        let action = self.cooldown_type.action();
        tokio::spawn(async move {
            if let Err(error) = SharedCooldownService::reset(
                &db,
                CooldownService::subject_type(Some(subject_id)),
                Some(subject_id),
                action,
            )
            .await
            {
                tracing::error!(error = ?error, "failed to release dropped cooldown lease");
            }
        });
    }
}
