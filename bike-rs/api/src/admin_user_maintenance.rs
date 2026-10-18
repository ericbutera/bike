use crate::{app_error::AppError, storage::AppStorage};
use bike_core::{
    archive_import::{enqueue_activity_archive_import_job, normalize_archive_url},
    auth::entities::users,
    entities::activity_archive_import_jobs,
};

pub async fn import_archive(
    state: &AppStorage,
    user_id: i32,
    archive_url: &str,
) -> Result<activity_archive_import_jobs::Model, AppError> {
    let archive_url = normalize_archive_url(archive_url)?;
    let user = users::Model::find_optional(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;
    user.ensure_enabled()
        .map_err(|error| AppError::forbidden(error.message))?;
    enqueue_activity_archive_import_job(&state.db, &state.tasks, user.id, &user.pid.to_string(), archive_url)
        .await
        .map_err(Into::into)
}
