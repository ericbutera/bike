pub use bike_core::auth::SessionService as AppSessionService;
pub use bike_core::jobs::{JobQueue as TaskQueue, QueuedJobReference as QueuedTaskReference};
use sea_orm::DatabaseConnection;

pub fn create_session_service(db: DatabaseConnection) -> AppSessionService {
    AppSessionService::new(db, bike_core::config::Config::get().jwt_secret.clone())
}
