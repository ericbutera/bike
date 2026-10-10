//! Bike's durable background queue and worker runtime.
pub mod batches;
pub mod diagnostics;
pub mod entities;
pub mod error;
pub mod execution;
pub mod history;
pub mod openapi;
pub mod pipeline;
pub mod queue;
pub mod storage;
pub mod task;

pub mod admin;
pub mod durable;
pub mod worker;

pub use entities::background_tasks;
pub use error::TaskError;
pub use queue::TaskQueue;
pub use storage::{TaskRecord, TaskStatus, TaskStorage};
pub use task::Task;

pub use durable::DurableStorage;

#[cfg(test)]
pub(crate) mod history_tests;

#[cfg(test)]
mod admin_tests;
