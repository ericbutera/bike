use super::*;

impl StravaOperations {
    #[tracing::instrument(
        name = "strava.sync.prepare",
        skip_all,
        fields(
            otel.name = "Prepare scheduled Strava syncs",
            bike.operation = "strava.sync.prepare",
            correlation_id = %request.correlation_id,
            after_connection_id = request.after_connection_id,
            connections_scanned = tracing::field::Empty,
            sync_events_prepared = tracing::field::Empty,
        )
    )]
    pub async fn scan_connections(&self, request: ScanRequest) -> Result<ScanPage, WorkflowError> {
        let connections = strava_connections::Model::scan_page(&self.db, request.after_connection_id).await?;
        let span = tracing::Span::current();
        span.record("connections_scanned", connections.len() as i64);
        let next_connection_id = (connections.len() == 20)
            .then(|| connections.last().map(|c| c.id))
            .flatten();
        let mut events = Vec::new();
        for connection in connections {
            if !super::super::scopes_allow_activity_import(&connection.scopes) {
                continue;
            }
            let user = bike_core::auth::entities::users::Model::find_by_id(&self.db, connection.user_id).await?;
            let mut event = StravaEvent::new(&user, &connection, super::super::events::StravaOperation::Sync);
            event.strava_event_id = uuid::Uuid::new_v5(&request.correlation_id, &connection.id.to_be_bytes());
            event.correlation_id = request.correlation_id;
            events.push(self.accept_child(event).await?);
        }
        span.record("sync_events_prepared", events.len() as i64);
        Ok(ScanPage {
            events,
            next_connection_id,
        })
    }
}
