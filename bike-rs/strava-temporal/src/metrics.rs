//! Temporal's supported Prometheus exporter; identifiers stay in logs and history.
use temporalio_common::telemetry::{
    start_prometheus_metric_exporter, PrometheusExporterOptions, TelemetryOptions,
};
use temporalio_sdk::runtime::RuntimeOptions;
pub struct MetricsServer {
    pub bound_addr: std::net::SocketAddr,
    pub abort_handle: tokio::task::AbortHandle,
}

impl Drop for MetricsServer {
    fn drop(&mut self) {
        self.abort_handle.abort();
    }
}

pub fn start(
    address: std::net::SocketAddr,
) -> Result<(RuntimeOptions, MetricsServer), crate::client::Error> {
    let server = start_prometheus_metric_exporter(
        PrometheusExporterOptions::builder()
            .socket_addr(address)
            .counters_total_suffix(true)
            .use_seconds_for_durations(true)
            .unit_suffix(true)
            .build(),
    )?;
    let meter: std::sync::Arc<dyn temporalio_common::telemetry::metrics::CoreMeter> = server.meter;
    let options = RuntimeOptions::builder()
        .telemetry_options(TelemetryOptions::builder().metrics(meter).build())
        .build()?;
    Ok((
        options,
        MetricsServer {
            bound_addr: server.bound_addr,
            abort_handle: server.abort_handle,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dropping_metrics_owner_aborts_its_task_on_error_or_shutdown() {
        let task = tokio::spawn(std::future::pending::<()>());
        let server = MetricsServer {
            bound_addr: "127.0.0.1:9092".parse().unwrap(),
            abort_handle: task.abort_handle(),
        };
        drop(server);
        assert!(task.await.unwrap_err().is_cancelled());
    }
}
