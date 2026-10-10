use super::{
    client::SnapshotClient,
    proto,
    types::{Point, RenderRequest},
    MapImageError, Theme, Variant,
};
use opentelemetry::trace::{SpanKind, TracerProvider};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use std::sync::{Arc, Mutex};
use tokio_stream::wrappers::TcpListenerStream;
use tracing::instrument::WithSubscriber;
use tracing_subscriber::prelude::*;

#[derive(Clone, Default)]
struct Worker {
    parents: Arc<Mutex<Vec<String>>>,
}

#[tonic::async_trait]
impl proto::map_service_server::MapService for Worker {
    async fn render(
        &self,
        request: tonic::Request<proto::RenderMapRequest>,
    ) -> Result<tonic::Response<proto::RenderMapResponse>, tonic::Status> {
        assert_eq!(
            request.metadata().get("authorization").unwrap(),
            "Bearer worker-secret"
        );
        assert!(request.metadata().contains_key("grpc-timeout"));
        self.parents.lock().unwrap().push(
            request
                .metadata()
                .get("traceparent")
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned(),
        );
        let input = request.into_inner();
        assert_eq!(input.points.len(), 2);
        if input.theme == "dark" {
            return Err(tonic::Status::unavailable("fixture failure"));
        }
        let png = if input.dpr == 2 {
            b"invalid-png".to_vec()
        } else {
            b"\x89PNG\r\n\x1a\nfixture-png".to_vec()
        };
        Ok(tonic::Response::new(proto::RenderMapResponse {
            png,
            cache_hit: false,
        }))
    }
}

#[tokio::test]
async fn snapshot_client_propagates_client_spans_and_records_failures() {
    opentelemetry::global::set_text_map_propagator(
        opentelemetry_sdk::propagation::TraceContextPropagator::new(),
    );
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("fixture")));
    let worker = Worker::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = proto::map_service_server::MapServiceServer::new(worker.clone());
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(server)
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    let client = SnapshotClient::new(format!("http://{address}"), "worker-secret".into()).unwrap();
    verify_snapshot_responses(&client)
        .with_subscriber(subscriber)
        .await;
    provider.force_flush().unwrap();
    let spans = exporter.get_finished_spans().unwrap();
    assert_eq!(spans.len(), 3);
    for (span, parent) in spans.iter().zip(worker.parents.lock().unwrap().iter()) {
        assert_eq!(span.span_kind, SpanKind::Client);
        assert_eq!(
            parent,
            &format!(
                "00-{}-{}-01",
                span.span_context.trace_id(),
                span.span_context.span_id()
            )
        );
        assert!(
            span.attributes
                .iter()
                .any(|value| value.key.as_str() == "rpc.method"
                    && value.value.to_string() == "Render")
        );
        assert!(!span
            .attributes
            .iter()
            .any(|value| value.key.as_str().contains("token")
                || value.key.as_str().contains("points")));
    }
    assert!(matches!(
        spans[1].status,
        opentelemetry::trace::Status::Error { .. }
    ));
    assert!(matches!(
        spans[2].status,
        opentelemetry::trace::Status::Error { .. }
    ));
    task.abort();
    provider.shutdown().unwrap();
}

async fn verify_snapshot_responses(client: &SnapshotClient) {
    let mut input = RenderRequest {
        theme: Theme::Light,
        variant: Variant::Full,
        dpr: 1,
        points: vec![
            Point {
                latitude: 45.0,
                longitude: -85.0,
            },
            Point {
                latitude: 45.1,
                longitude: -85.1,
            },
        ],
    };
    assert!(client
        .snapshot(&input)
        .await
        .unwrap()
        .starts_with(b"\x89PNG"));
    input.theme = Theme::Dark;
    assert!(matches!(
        client.snapshot(&input).await,
        Err(MapImageError::Snapshot(_))
    ));
    input.theme = Theme::Light;
    input.dpr = 2;
    assert!(matches!(
        client.snapshot(&input).await,
        Err(MapImageError::InvalidPNG)
    ));
}
