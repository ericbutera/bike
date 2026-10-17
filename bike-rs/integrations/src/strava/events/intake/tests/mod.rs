use super::*;

mod requests;
mod webhooks;

struct Intake {
    service: StravaIntake,
    requests: std::sync::Arc<std::sync::Mutex<bike_core::test_support::temporal_client::Requests>>,
}

impl Intake {
    async fn new(db: &DatabaseConnection) -> Self {
        let requests = Default::default();
        let client = bike_core::test_support::temporal_client::client(std::sync::Arc::clone(&requests)).await;
        Self {
            service: StravaIntake::with_client(db.clone(), client),
            requests,
        }
    }
}
