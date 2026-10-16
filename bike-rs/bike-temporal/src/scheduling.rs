//! Native Signal-With-Start is a client RPC, not a Workflow command. These
//! retryable Activities make that atomic handoff from any parent workflow.
//! They await acceptance only; the independently keyed output owns its timers.
use crate::{activities::activity_error, client, workflows::DebouncePolicy};
use bike_core::bike_tasks::{validate_revision, FitnessChange, HeatmapChange, SegmentChange};
use std::sync::Arc;
use temporalio_client::Client;
use temporalio_macros::activities;
use temporalio_sdk::activities::{ActivityContext, ActivityError};

pub struct SchedulingActivities {
    client: Client,
    policy: DebouncePolicy,
}

impl SchedulingActivities {
    pub fn new(client: Client, policy: DebouncePolicy) -> Self {
        Self { client, policy }
    }
}

#[activities]
impl SchedulingActivities {
    #[activity(name = "schedule_fitness_freshness")]
    pub async fn schedule_fitness_freshness(
        self: Arc<Self>,
        _ctx: ActivityContext,
        change: FitnessChange,
    ) -> Result<(), ActivityError> {
        client::request_fitness_freshness(&self.client, change, self.policy.clone())
            .await
            .map_err(rpc_error)
    }

    #[activity(name = "schedule_segment_analytics")]
    pub async fn schedule_segment_analytics(
        self: Arc<Self>,
        _ctx: ActivityContext,
        changes: Vec<SegmentChange>,
    ) -> Result<(), ActivityError> {
        for change in &changes {
            validate_revision(change.revision).map_err(activity_error)?;
        }
        for change in changes {
            client::request_segment_analytics(&self.client, change, self.policy.clone())
                .await
                .map_err(rpc_error)?;
        }
        Ok(())
    }

    #[activity(name = "schedule_heatmap")]
    pub async fn schedule_heatmap(
        self: Arc<Self>,
        _ctx: ActivityContext,
        change: HeatmapChange,
    ) -> Result<(), ActivityError> {
        client::request_heatmap(&self.client, change, self.policy.clone())
            .await
            .map_err(rpc_error)
    }
}

fn rpc_error(error: client::Error) -> ActivityError {
    activity_error(std::io::Error::other(error.to_string()))
}
