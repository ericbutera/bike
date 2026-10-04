use tracing_subscriber::EnvFilter;

pub fn init_json_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();
}
