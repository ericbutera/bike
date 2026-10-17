//! Cleanup eligibility only. A candidate is never authorization to unlink bytes.
use crate::object_store::{ObjectOwner, ObjectRef};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Succeeded,
    Failed,
    Canceled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ObjectLifetime {
    Canonical,
    Unaccepted {
        last_attempt_at: DateTime<Utc>,
    },
    Open,
    Closed {
        final_chain_closed_at: DateTime<Utc>,
        outcome: Outcome,
    },
}

/// Persist with the object's registry, independently of Temporal history retention.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetentionRecord {
    pub object: ObjectRef,
    pub owner: ObjectOwner,
    pub lifetime: ObjectLifetime,
}

#[derive(Clone, Debug)]
pub struct RetentionPolicy {
    abandoned: TimeDelta,
    succeeded: TimeDelta,
    failed: TimeDelta,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            abandoned: TimeDelta::hours(24),
            succeeded: TimeDelta::days(14),
            failed: TimeDelta::days(90),
        }
    }
}

impl RetentionPolicy {
    pub fn new(
        abandoned: TimeDelta,
        succeeded: TimeDelta,
        failed: TimeDelta,
    ) -> Result<Self, crate::workflow_error::WorkflowError> {
        if [abandoned, succeeded, failed]
            .into_iter()
            .any(|ttl| ttl <= TimeDelta::zero())
        {
            return Err(crate::workflow_error::WorkflowError::bad_request(
                "Object TTLs must be positive",
            ));
        }
        Ok(Self {
            abandoned,
            succeeded,
            failed,
        })
    }

    fn expires_at(&self, lifetime: &ObjectLifetime) -> Option<DateTime<Utc>> {
        let (at, ttl) = match lifetime {
            ObjectLifetime::Canonical | ObjectLifetime::Open => return None,
            ObjectLifetime::Unaccepted { last_attempt_at } => (*last_attempt_at, self.abandoned),
            ObjectLifetime::Closed {
                final_chain_closed_at,
                outcome,
            } => {
                let ttl = if *outcome == Outcome::Succeeded {
                    self.succeeded
                } else {
                    self.failed
                };
                (*final_chain_closed_at, ttl)
            }
        };
        at.checked_add_signed(ttl)
    }
}

/// A complete inventory includes active/retained histories, database references,
/// retry pins and the writer/pin generation. Recheck it under deletion coordination.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReferenceInventory {
    pub complete: bool,
    pub workflow_reference: bool,
    pub database_reference: bool,
    pub retry_pin: bool,
}

impl ReferenceInventory {
    fn referenced(self) -> bool {
        [self.workflow_reference, self.database_reference, self.retry_pin].contains(&true)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetentionDecision {
    ForeignOwner,
    Protected,
    InventoryUnavailable,
    NotExpired,
    Candidate,
}

impl RetentionRecord {
    pub(super) fn decision(
        &self,
        sweeper: ObjectOwner,
        now: DateTime<Utc>,
        policy: &RetentionPolicy,
        inventory: ReferenceInventory,
    ) -> RetentionDecision {
        if self.owner != sweeper || !sweeper.owns(&self.object.path) {
            return RetentionDecision::ForeignOwner;
        }
        if !inventory.complete {
            return RetentionDecision::InventoryUnavailable;
        }
        if inventory.referenced() {
            return RetentionDecision::Protected;
        }
        match self.lifetime {
            ObjectLifetime::Canonical | ObjectLifetime::Open => RetentionDecision::Protected,
            _ => match policy.expires_at(&self.lifetime) {
                Some(expiry) if expiry <= now => RetentionDecision::Candidate,
                _ => RetentionDecision::NotExpired,
            },
        }
    }
}

#[cfg(test)]
mod tests;
