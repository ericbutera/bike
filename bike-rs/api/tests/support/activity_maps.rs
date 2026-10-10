use super::support;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
    Router,
};
use bike_core::{
    activity_maps::{proto, MapImageService},
    entities::activities,
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::task::JoinHandle;
use tower::ServiceExt;

const IMAGE_PATH: &str = "/api/activity-map-images/full/1?activityId=1109&theme=dark&dpr=2";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfixture-png";

struct Worker {
    calls: AtomicUsize,
    fail: AtomicUsize,
}
struct Fixture {
    worker: Arc<Worker>,
    server: JoinHandle<()>,
    cache: tempfile::TempDir,
    app: Router,
    db: sea_orm::DatabaseConnection,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

struct SnapshotWorker(Arc<Worker>);

#[tonic::async_trait]
impl proto::map_service_server::MapService for SnapshotWorker {
    async fn render(
        &self,
        call: tonic::Request<proto::RenderMapRequest>,
    ) -> Result<tonic::Response<proto::RenderMapResponse>, tonic::Status> {
        assert_eq!(
            call.metadata().get("authorization").unwrap(),
            "Bearer snapshot-secret"
        );
        assert!(!call.metadata().contains_key("cookie"));
        assert!(!call.metadata().contains_key("x-bike-synthetic-key"));
        assert!(call.metadata().contains_key("grpc-timeout"));
        let request = call.into_inner();
        assert_eq!(request.theme, "dark");
        assert_eq!(request.variant, "full");
        assert_eq!(request.dpr, 2);
        assert!(request.points.len() >= 2);
        self.0.calls.fetch_add(1, Ordering::SeqCst);
        if self.0.fail.load(Ordering::SeqCst) != 0 {
            return Err(tonic::Status::unavailable("fixture worker failure"));
        }
        Ok(tonic::Response::new(proto::RenderMapResponse {
            png: PNG.to_vec(),
            cache_hit: false,
        }))
    }
}

async fn fixture(local_admin: bool) -> Fixture {
    let worker = Arc::new(Worker {
        calls: AtomicUsize::new(0),
        fail: AtomicUsize::new(0),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let service =
        proto::map_service_server::MapServiceServer::new(SnapshotWorker(Arc::clone(&worker)));
    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(service)
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    let cache = tempfile::tempdir().unwrap();
    let service = Arc::new(
        MapImageService::new(
            format!("http://{address}"),
            "snapshot-secret".into(),
            cache.path().into(),
            Duration::from_secs(60),
        )
        .await
        .unwrap(),
    );
    let (app, db) = support::map_image_platform_app(local_admin, service).await;
    Fixture {
        worker,
        server,
        cache,
        app,
        db,
    }
}

async fn image(app: &Router, path: &str, etag: Option<&str>) -> Response {
    let mut request = Request::builder().uri(path);
    if let Some(etag) = etag {
        request = request.header("if-none-match", etag);
    }
    app.clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn map_image_reads_owned_geometry_and_caches_private_png() {
    let fixture = fixture(true).await;
    let response = image(&fixture.app, IMAGE_PATH, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/png");
    assert_eq!(response.headers()["cache-control"], "private, no-cache");
    assert_eq!(response.headers()["vary"], "Cookie, Authorization");
    assert_eq!(response.headers()["x-map-cache"], "miss");
    let etag = response.headers()["etag"].to_str().unwrap().to_owned();
    assert_eq!(
        axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap(),
        PNG
    );
    let response = image(&fixture.app, IMAGE_PATH, Some(&etag)).await;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(response.headers()["x-map-cache"], "hit");
    assert!(axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(fixture.worker.calls.load(Ordering::SeqCst), 1);
    assert_eq!(std::fs::read_dir(fixture.cache.path()).unwrap().count(), 1);
    let mut owner: bike_core::auth::entities::users::ActiveModel =
        bike_core::auth::entities::users::Entity::find_by_id(1)
            .one(&fixture.db)
            .await
            .unwrap()
            .unwrap()
            .into();
    owner.id = Set(2);
    owner.pid = Set(uuid::Uuid::new_v4());
    owner.email = Set("other@bike.local".into());
    owner.api_key = Set("other-key".into());
    owner.insert(&fixture.db).await.unwrap();
    let mut activity: activities::ActiveModel = activities::Entity::find_by_id(1109)
        .one(&fixture.db)
        .await
        .unwrap()
        .unwrap()
        .into();
    activity.user_id = Set(2);
    activity.update(&fixture.db).await.unwrap();
    assert_eq!(
        image(&fixture.app, IMAGE_PATH, Some(&etag)).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(fixture.worker.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn map_image_auth_and_options_are_checked_before_snapshot() {
    let protected = fixture(false).await;
    assert_eq!(
        image(&protected.app, IMAGE_PATH, None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(protected.worker.calls.load(Ordering::SeqCst), 0);
    let fixture = fixture(true).await;
    for path in [
        IMAGE_PATH.replace("dpr=2", "dpr=3"),
        IMAGE_PATH.replace("theme=dark", "theme=unknown"),
        IMAGE_PATH.replace("/full/1", "/other/1"),
        IMAGE_PATH.replace("/full/1", "/full/9"),
        IMAGE_PATH.replace("activityId=1109", "activityId=0"),
    ] {
        assert_eq!(
            image(&fixture.app, &path, None).await.status(),
            StatusCode::BAD_REQUEST,
            "{path}"
        );
    }
    let missing = IMAGE_PATH.replace("activityId=1109", "activityId=999");
    assert_eq!(
        image(&fixture.app, &missing, None).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(fixture.worker.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn map_image_coalesces_concurrent_misses_and_expires_idle_files() {
    let fixture = fixture(true).await;
    let (first, second) = tokio::join!(
        image(&fixture.app, IMAGE_PATH, None),
        image(&fixture.app, IMAGE_PATH, None)
    );
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(fixture.worker.calls.load(Ordering::SeqCst), 1);
    let file = std::fs::read_dir(fixture.cache.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::File::open(file)
        .unwrap()
        .set_times(
            std::fs::FileTimes::new()
                .set_modified(std::time::SystemTime::now() - Duration::from_secs(61)),
        )
        .unwrap();
    assert_eq!(
        image(&fixture.app, IMAGE_PATH, None).await.status(),
        StatusCode::OK
    );
    assert_eq!(fixture.worker.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn map_image_missing_geometry_does_not_call_snapshot_worker() {
    let fixture = fixture(true).await;
    let mut activity: activities::ActiveModel = activities::Entity::find_by_id(1109)
        .one(&fixture.db)
        .await
        .unwrap()
        .unwrap()
        .into();
    activity.derived_data_json = Set(None);
    activity.update(&fixture.db).await.unwrap();
    assert_eq!(
        image(&fixture.app, IMAGE_PATH, None).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(fixture.worker.calls.load(Ordering::SeqCst), 0);
}
