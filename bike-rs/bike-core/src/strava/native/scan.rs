use super::*;

impl StravaOperations {
    pub async fn scan_connections(&self, request: ScanRequest) -> Result<ScanPage, WorkflowError> {
        let connections =
            strava_connections::Model::scan_page(&self.db, request.after_connection_id).await?;
        let next_connection_id = (connections.len() == 20)
            .then(|| connections.last().map(|c| c.id))
            .flatten();
        let mut events = Vec::new();
        for connection in connections {
            if !super::super::scopes_allow_activity_import(&connection.scopes) {
                continue;
            }
            let user =
                crate::auth::entities::users::Model::find_by_id(&self.db, connection.user_id)
                    .await?;
            let mut event = StravaEvent::new(
                &user,
                &connection,
                super::super::events::StravaOperation::Sync,
            );
            event.strava_event_id =
                uuid::Uuid::new_v5(&request.correlation_id, &connection.id.to_be_bytes());
            event.correlation_id = request.correlation_id;
            events.push(self.accept_child(event).await?);
        }
        Ok(ScanPage {
            events,
            next_connection_id,
        })
    }
}
