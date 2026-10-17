mod adapter;
mod error;
pub mod native;
pub use adapter::{JobQueue, QueuedJobReference};
pub use error::TaskError;
