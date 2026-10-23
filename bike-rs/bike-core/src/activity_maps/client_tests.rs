use super::{
    client::SnapshotClient,
    proto,
    types::{Point, RenderRequest},
    MapImageError, Theme, Variant,
};
use tokio_stream::wrappers::TcpListenerStream;

struct Worker;

#[tonic::async_trait]
impl proto::map_service_server::MapService for Worker {
    async fn render(
        &self,
        request: tonic::Request<proto::RenderMapRequest>,
    ) -> Result<tonic::Response<proto::RenderMapResponse>, tonic::Status> {
        assert_eq!(request.metadata().get("authorization").unwrap(), "Bearer worker-secret");
        assert!(request.metadata().contains_key("grpc-timeout"));
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
        Ok(tonic::Response::new(proto::RenderMapResponse { png, cache_hit: false }))
    }
}

#[tokio::test]
async fn snapshot_client_returns_png_and_rejects_failed_or_invalid_snapshots() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = proto::map_service_server::MapServiceServer::new(Worker);
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(server)
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    let client = SnapshotClient::new(format!("http://{address}"), "worker-secret".into()).unwrap();
    verify_snapshot_responses(&client).await;
    task.abort();
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
    assert!(client.snapshot(&input).await.unwrap().starts_with(b"\x89PNG"));
    input.theme = Theme::Dark;
    assert!(matches!(client.snapshot(&input).await, Err(MapImageError::Snapshot(_))));
    input.theme = Theme::Light;
    input.dpr = 2;
    assert!(matches!(client.snapshot(&input).await, Err(MapImageError::InvalidPNG)));
}
