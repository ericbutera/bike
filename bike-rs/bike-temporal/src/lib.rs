//! Native Bike coordination. No SQL queue, polling, or orchestration tables.
pub mod activities;
pub mod client;
pub mod scheduling;
pub mod workflows;

pub const TASK_QUEUE: &str = "bike-tasks";

#[cfg(test)]
mod tests;
