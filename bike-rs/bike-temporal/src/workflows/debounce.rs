//! Temporal's signal + resettable timer pattern. This buffers one typed output,
//! without scheduling work or maintaining an independent running-task state.
use bike_core::bike_tasks::{FitnessChange, HeatmapChange, Priority, Publication, SegmentChange};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use temporalio_sdk::{
    ApplicationFailure, CancellableFuture, ContinueAsNewOptions, SyncWorkflowContext, WorkflowContext, WorkflowResult,
};

pub trait RebuildRequest: Clone + PartialEq + Serialize + 'static {
    fn revision(&self) -> u64;
    fn priority(&self) -> Priority;
    fn same_resource(&self, other: &Self) -> bool;
    fn merge(&mut self, other: Self);
}

impl RebuildRequest for FitnessChange {
    fn revision(&self) -> u64 {
        self.revision
    }
    fn priority(&self) -> Priority {
        self.priority
    }
    fn same_resource(&self, other: &Self) -> bool {
        self.user_id == other.user_id
    }
    fn merge(&mut self, other: Self) {
        self.revision = self.revision.max(other.revision);
        self.priority = self.priority.min(other.priority);
        self.dirty_from = self.dirty_from.min(other.dirty_from);
    }
}

impl RebuildRequest for SegmentChange {
    fn revision(&self) -> u64 {
        self.revision
    }
    fn priority(&self) -> Priority {
        self.priority
    }
    fn same_resource(&self, other: &Self) -> bool {
        self.segment_id == other.segment_id
    }
    fn merge(&mut self, other: Self) {
        self.revision = self.revision.max(other.revision);
        self.priority = self.priority.min(other.priority);
    }
}

impl RebuildRequest for HeatmapChange {
    fn revision(&self) -> u64 {
        self.generation
    }
    fn priority(&self) -> Priority {
        self.priority
    }
    fn same_resource(&self, other: &Self) -> bool {
        (self.user_id, self.activity_id) == (other.user_id, other.activity_id)
    }
    fn merge(&mut self, other: Self) {
        self.generation = self.generation.max(other.generation);
        self.priority = self.priority.min(other.priority);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DebouncePolicy {
    pub new_activity_quiet: TimeDelta,
    pub new_activity_cap: TimeDelta,
    pub background_quiet: TimeDelta,
    pub background_cap: TimeDelta,
}

impl Default for DebouncePolicy {
    fn default() -> Self {
        Self {
            new_activity_quiet: TimeDelta::seconds(2),
            new_activity_cap: TimeDelta::seconds(10),
            background_quiet: TimeDelta::seconds(60),
            background_cap: TimeDelta::minutes(5),
        }
    }
}

impl DebouncePolicy {
    fn deadline(&self, priority: Priority, first: DateTime<Utc>, at: DateTime<Utc>) -> WorkflowResult<DateTime<Utc>> {
        let (quiet, cap) = match priority {
            Priority::NewActivity => (self.new_activity_quiet, self.new_activity_cap),
            Priority::Background => (self.background_quiet, self.background_cap),
        };
        if quiet <= TimeDelta::zero() || cap < quiet {
            return Err(ApplicationFailure::new("Debounce requires 0 < quiet <= cap").into());
        }
        Ok((at + quiet).min(first + cap))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Pending<R> {
    pub request: R,
    first: DateTime<Utc>,
    deadline: DateTime<Utc>,
}

/// Continuation input for a specific named workflow, never a task-type switch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RebuildInput<R> {
    resource: R,
    policy: DebouncePolicy,
    published: u64,
    blocked: u64,
    pending: Option<Pending<R>>,
}

impl<R> RebuildInput<R> {
    pub fn new(resource: R, policy: DebouncePolicy) -> Self {
        Self {
            resource,
            policy,
            published: 0,
            blocked: 0,
            pending: None,
        }
    }
}

impl<R: RebuildRequest> RebuildInput<R> {
    fn accept(&mut self, request: R, at: DateTime<Utc>) -> WorkflowResult<()> {
        if !self.resource.same_resource(&request) || request.revision() == 0 {
            return Err(ApplicationFailure::new("Change needs this resource and a positive revision").into());
        }
        // A late notification can describe an independently committed activity.
        // A numeric watermark cannot prove that its effect was included already.
        // Coalesce pending requests; rebuilding current data again is harmless.
        let Some(pending) = &mut self.pending else {
            self.pending = Some(Pending {
                deadline: self.policy.deadline(request.priority(), at, at)?,
                first: at,
                request,
            });
            return Ok(());
        };
        let previous = pending.request.clone();
        pending.request.merge(request.clone());
        if pending.request == previous {
            return Ok(());
        }
        if request.priority() <= previous.priority() {
            let deadline = self.policy.deadline(request.priority(), pending.first, at)?;
            pending.deadline = if request.priority() < previous.priority() {
                deadline.min(pending.deadline)
            } else {
                deadline
            };
        }
        Ok(())
    }

    pub(super) fn complete(&mut self, batch: Pending<R>, publication: Publication) {
        match publication {
            Publication::Published => {
                self.published = batch.request.revision();
                self.blocked = 0;
            }
            Publication::Superseded => {
                self.blocked = batch.request.revision();
                self.pending = Some(batch);
            }
        }
    }
}

pub(super) struct Signals<R> {
    received: Vec<(R, DateTime<Utc>)>,
    version: u64,
    draining: bool,
}

impl<R> Default for Signals<R> {
    fn default() -> Self {
        Self {
            received: vec![],
            version: 0,
            draining: false,
        }
    }
}

impl<R> Signals<R> {
    pub fn drain(&mut self) {
        self.draining = true;
        self.version += 1;
    }

    pub fn push<W>(&mut self, ctx: &SyncWorkflowContext<W>, request: R) {
        let at = ctx.workflow_time().expect("Temporal activation has workflow time");
        self.received.push((request, at.into()));
        self.version += 1;
    }
}

pub(super) fn should_finish<W, R>(
    ctx: &WorkflowContext<W>,
    input: &RebuildInput<R>,
    read: fn(&W) -> &Signals<R>,
) -> bool {
    // Existing histories may contain another timer after publication. Keep
    // replaying those commands until an explicit drain retires that old run.
    let finite = ctx.patched("bike-output-drain-v1");
    let drained = input.pending.is_none() && ctx.state(|state| read(state).received.is_empty());
    finite && drained
}

pub(super) async fn next<W: temporalio_sdk::workflows::WorkflowImplementation + 'static, R: RebuildRequest>(
    ctx: &mut WorkflowContext<W>,
    input: &mut RebuildInput<R>,
    read: fn(&W) -> &Signals<R>,
    write: fn(&mut W) -> &mut Signals<R>,
) -> WorkflowResult<Option<Pending<R>>>
where
    W::Run: temporalio_workflow::common::WorkflowDefinition<Input = RebuildInput<R>>,
{
    loop {
        if ctx.cancellation_token().is_cancelled() {
            return Err(temporalio_sdk::WorkflowTermination::cancelled());
        }
        let received = ctx.state_mut(|state| std::mem::take(&mut write(state).received));
        for (request, at) in received {
            input.accept(request, at)?;
        }
        if input.pending.is_none() && ctx.state(|state| read(state).draining) {
            return Ok(None);
        }
        if ctx.continue_as_new_suggested() {
            ctx.continue_as_new(input.clone(), ContinueAsNewOptions::default())?;
            unreachable!();
        }
        let version = ctx.state(|state| read(state).version);
        let deadline = input
            .pending
            .as_ref()
            .filter(|p| p.request.revision() > input.blocked)
            .map(|p| p.deadline);
        let Some(deadline) = deadline else {
            ctx.wait_condition(move |state: &W| read(state).version != version)
                .await?;
            continue;
        };
        let now = DateTime::<Utc>::from(ctx.workflow_time().expect("Temporal activation has workflow time"));
        if deadline <= now {
            return Ok(input.pending.take());
        }
        let mut timer = std::pin::pin!(ctx.timer((deadline - now).to_std().unwrap_or(Duration::ZERO)));
        temporalio_sdk::workflows::select! {
            _ = &mut timer => {}
            result = ctx.wait_condition(move |state: &W| read(state).version != version) => { timer.cancel(); result?; }
        }
    }
}
