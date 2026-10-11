//! Bike-owned Strava workflows on the existing durable worker and import DAG.
mod activity;
mod child;
mod collection;
pub(crate) mod import_objects;
mod inbox;
pub(crate) mod intents;
mod revocation;
mod stage;
pub use inbox::{
    accept_webhook, active_user_collection, queue_disconnect, queue_reconciliation, queue_sync,
    queue_user_sync, resume_at,
};

/// New-user onboarding must never automatically request more than 15 days.
pub const INITIAL_BACKFILL_DAYS: i64 = 15;

pub fn provider_work_enabled() -> bool {
    std::env::var("STRAVA_PROVIDER_WORK_ENABLED").as_deref() != Ok("false")
}

use crate::{
    config::Config,
    entities::{strava_connections, strava_workflows},
    strava_client::{RequestArchive, StravaApiClient},
    strava_objects::ObjectStore,
    workflow_error::WorkflowError,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, EntityTrait, TransactionTrait};
use serde::{Deserialize, Serialize};
use tracing::Instrument;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Source {
    #[serde(default)]
    pub revoke_remote: bool,
    pub generation: chrono::DateTime<chrono::Utc>,
    pub event_time: i64,
    pub parent: Option<String>,
    pub activity: Option<serde_json::Value>,
    pub raw: Option<crate::strava_objects::ObjectRef>,
    pub after: Option<i64>,
}

pub(crate) struct Run<'a> {
    db: &'a DatabaseConnection,
    row: strava_workflows::Model,
    source: Source,
    connection: strava_connections::Model,
    client: StravaApiClient,
    store: ObjectStore,
    token: String,
}

pub fn graph(action: &str) -> Result<Vec<&'static str>, WorkflowError> {
    let stages = match action {
        "insert" | "update" | "upsert" => {
            vec!["activity", "streams", "normalize", "apply", "derived"]
        }
        "delete" => vec!["capture_delete", "delete", "derived"],
        "sync" | "backfill" => vec!["discover", "children", "complete"],
        "deauthorize" => vec!["capture_cleanup", "revoke", "cleanup"],
        _ => return Err(WorkflowError::bad_request("Unknown Strava workflow action")),
    };
    let mut dag = petgraph::graph::DiGraph::<&str, ()>::new();
    let nodes: Vec<_> = stages.iter().map(|stage| dag.add_node(stage)).collect();
    for edge in nodes.windows(2) {
        dag.add_edge(edge[0], edge[1], ());
    }
    petgraph::algo::toposort(&dag, None)
        .map(|order| order.into_iter().map(|node| dag[node]).collect())
        .map_err(|_| WorkflowError::internal("Strava workflow graph contains a cycle"))
}

pub async fn execute(db: &DatabaseConnection, id: &str) -> Result<(), WorkflowError> {
    let row = strava_workflows::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| WorkflowError::not_found("Strava workflow was not found"))?;
    if row.status == "completed" {
        return Ok(());
    }
    if !provider_work_enabled() {
        return Err(WorkflowError::too_many_requests(
            "Strava provider work is paused for cutover",
            Some(chrono::Utc::now() + chrono::Duration::minutes(1)),
        ));
    }
    // One connection owns refresh and provider work. PostgreSQL releases this
    // transaction-scoped lease automatically if the worker dies.
    let lease = db.begin().await?;
    if db.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
        lease
            .execute_unprepared(&format!(
                "SELECT pg_advisory_xact_lock(7391, {})",
                row.connection_id
            ))
            .await?;
    }
    // A concurrent delivery may have completed while this worker waited.
    let mut row = strava_workflows::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| WorkflowError::not_found("Strava workflow was not found"))?;
    if row.status == "completed" {
        lease.commit().await?;
        return Ok(());
    }
    if row.checkpoint.get("pause").is_some() {
        let mut checkpoint = row.checkpoint.clone();
        checkpoint
            .as_object_mut()
            .ok_or_else(|| WorkflowError::internal("Invalid Strava workflow checkpoint"))?
            .remove("pause");
        row = row.checkpoint(db, checkpoint).await?;
    }
    let connection = strava_connections::Entity::find_by_id(row.connection_id)
        .one(db)
        .await?;
    let source: Source = serde_json::from_value(row.source.clone()).map_err(stage::json_error)?;
    let Some(connection) =
        connection.filter(|connection| connection.created_at == source.generation)
    else {
        row.finish(db, None).await?;
        return Ok(());
    };
    let client = StravaApiClient::new(db, Config::get())?;
    if row.action != "deauthorize"
        && strava_workflows::Model::pending_revocation(db, &connection)
            .await?
            .is_some()
    {
        row.finish(db, None).await?;
        lease.commit().await?;
        return Ok(());
    }
    let span = tracing::info_span!("strava.workflow.initialize", correlation_id = %row.correlation_id(), workflow_id = %row.id, parent_workflow_id = source.parent.as_deref(), strava_activity_id = row.activity_id, athlete_id = row.athlete_id, trace_id = tracing::field::Empty, span_id = tracing::field::Empty);
    crate::observability::record_span_trace_context(&span);
    let result = intents::scope(
        &row.id,
        execute_owned(db, row.clone(), source, connection, client).instrument(span),
    )
    .await;
    row.finish(db, result.as_ref().err().map(|error| error.message.clone()))
        .await?;
    lease.commit().await?;
    result
}

async fn execute_owned(
    db: &DatabaseConnection,
    row: strava_workflows::Model,
    source: Source,
    connection: strava_connections::Model,
    client: StravaApiClient,
) -> Result<(), WorkflowError> {
    if let Some(raw) = &source.raw {
        ObjectStore::new(&Config::get().uploads_dir)
            .read(raw)
            .await?;
    }
    let connection = if matches!(row.action.as_str(), "delete" | "deauthorize") {
        connection
    } else {
        crate::strava::ensure_fresh_access_token(db, &client, connection).await?
    };
    let token = crate::strava_credentials::open(&connection.access_token)?;
    let run = Run {
        db,
        store: ObjectStore::new(&Config::get().uploads_dir),
        row,
        source,
        connection,
        client,
        token,
    };
    Box::pin(run.execute()).await
}

impl Run<'_> {
    async fn execute(&self) -> Result<(), WorkflowError> {
        let span = tracing::info_span!("strava.workflow", correlation_id = %self.row.correlation_id(), workflow_id = %self.row.id, parent_workflow_id = self.source.parent.as_deref(), action = %self.row.action, athlete_id = self.row.athlete_id, strava_activity_id = self.row.activity_id, connection_id = self.row.connection_id, trace_id = tracing::field::Empty, span_id = tracing::field::Empty);
        crate::observability::record_span_trace_context(&span);
        async {
            match self.row.action.as_str() {
                "sync" | "backfill" => self.collection().await,
                "deauthorize" => self.deauthorize().await,
                _ => self.activity().await,
            }
        }
        .instrument(span)
        .await
    }

    fn archive(&self, activity_id: Option<i64>, revision: &str, resource: &str) -> RequestArchive {
        let key = match activity_id {
            Some(id) => format!(
                "strava/{}/{id}/raw/{revision}/{resource}",
                self.connection.athlete_id
            ),
            None => format!(
                "strava/{}/runs/{}/raw/{resource}",
                self.connection.athlete_id, self.row.id
            ),
        };
        RequestArchive {
            store: self.store.clone(),
            key,
        }
    }
}
