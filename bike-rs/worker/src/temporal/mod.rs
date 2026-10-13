mod activities;
mod connection;
mod options;
pub mod workflows;

pub use connection::{connect, worker, TemporalConfig};
pub use workflows::{StravaActivityDelete, StravaActivityUpsert};

#[cfg(test)]
mod tests;
