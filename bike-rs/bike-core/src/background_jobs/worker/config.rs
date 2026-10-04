use std::env;

#[derive(Debug, Clone, Copy)]
pub struct WorkerConfigDefaults {
    pub metrics_port: u16,
    pub batch_size: u64,
    pub poll_interval_secs: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct WorkerConfig {
    pub metrics_port: u16,
    pub batch_size: u64,
    pub poll_interval_secs: u64,
}

impl WorkerConfig {
    pub fn from_env(defaults: WorkerConfigDefaults) -> Self {
        Self {
            metrics_port: parse_env("METRICS_PORT", defaults.metrics_port),
            batch_size: parse_env("WORKER_BATCH_SIZE", defaults.batch_size),
            poll_interval_secs: parse_env("WORKER_POLL_INTERVAL", defaults.poll_interval_secs),
        }
    }
}

fn parse_env<T>(key: &str, default: T) -> T
where
    T: std::str::FromStr + Copy,
{
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<T>().ok())
        .unwrap_or(default)
}
