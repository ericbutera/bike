use super::{cache::Cache, types::RenderRequest};
use super::{types::MapImageError, MapImageService, MapOptions, Theme, Variant};
use std::{
    fs::{self, File, FileTimes},
    time::{Duration, SystemTime},
};

fn activity_fixture() -> crate::entities::activities::Model {
    let mut activity: crate::entities::activities::Model = serde_json::from_value(serde_json::json!({
        "id": 7, "user_id": 1, "title": "Map fixture", "sport": "ride", "source": "manual_upload", "activity_type": "training",
        "started_at": "2026-10-09T12:00:00Z", "created_at": "2026-10-09T12:00:00Z", "updated_at": "2026-10-09T12:00:00Z",
    })).unwrap();
    let data = crate::activity_details::ActivityDerivedData {
        route_points: serde_json::from_value(serde_json::json!([
            {"elapsed_seconds":0,"latitude":45.0,"longitude":-85.0},
            {"elapsed_seconds":60,"latitude":45.1,"longitude":-85.1}
        ]))
        .unwrap(),
        ..Default::default()
    };
    activity.derived_data_json =
        Some(crate::activity_details::serialize_derived_activity_data(&data).unwrap());
    activity
}

fn database(activity: Option<crate::entities::activities::Model>) -> sea_orm::DatabaseConnection {
    sea_orm::MockDatabase::new(sea_orm::DatabaseBackend::Postgres)
        .append_query_results([activity.into_iter().collect::<Vec<_>>()])
        .into_connection()
}

#[tokio::test]
async fn image_workflow_reuses_cache_without_contacting_worker() {
    let directory = tempfile::tempdir().unwrap();
    let service = MapImageService::new(
        "http://unused.invalid".into(),
        String::new(),
        directory.path().into(),
        Duration::from_secs(60),
    )
    .await
    .unwrap();
    let activity = activity_fixture();
    let route = crate::activity_details::deserialize_derived_activity_data(
        activity.derived_data_json.as_ref(),
    )
    .route_points;
    let options = MapOptions {
        theme: Theme::Light,
        dpr: 1,
    };
    let request = RenderRequest::from_route(route, Variant::Full, options).unwrap();
    let cache = Cache::new(directory.path().into(), Duration::from_secs(60))
        .await
        .unwrap();
    cache
        .write(request.cache_key(), b"\x89PNG\r\n\x1a\ncached-png".to_vec())
        .await
        .unwrap();
    let image = service
        .image(&database(Some(activity)), 1, 7, Variant::Full, "1", options)
        .await
        .unwrap();
    assert!(image.cache_hit);
    assert!(image.etag.starts_with('"'));
    service.prune().await.unwrap();
    assert!(matches!(
        service
            .image(&database(None), 1, 7, Variant::Full, "1", options)
            .await,
        Err(MapImageError::NotFound)
    ));
    assert!(matches!(
        service
            .image(&database(None), 1, 0, Variant::Full, "1", options)
            .await,
        Err(MapImageError::Invalid)
    ));
}

#[tokio::test]
async fn image_workflow_calls_and_caches_authenticated_snapshot_worker() {
    let worker = super::proto::map_service_server::MapServiceServer::new(FixtureWorker);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(worker)
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    let directory = tempfile::tempdir().unwrap();
    let service = std::sync::Arc::new(
        MapImageService::new(
            format!("http://{address}"),
            "worker-secret".into(),
            directory.path().into(),
            Duration::from_secs(60),
        )
        .await
        .unwrap(),
    );
    let image = service
        .image(
            &database(Some(activity_fixture())),
            1,
            7,
            Variant::Full,
            "1",
            MapOptions {
                theme: Theme::Light,
                dpr: 1,
            },
        )
        .await
        .unwrap();
    assert!(!image.cache_hit);
    assert_eq!(image.png, b"\x89PNG\r\n\x1a\nworker-png");
    service.maintain();
    tokio::task::yield_now().await;
    task.abort();
}

struct FixtureWorker;

#[tonic::async_trait]
impl super::proto::map_service_server::MapService for FixtureWorker {
    async fn render(
        &self,
        request: tonic::Request<super::proto::RenderMapRequest>,
    ) -> Result<tonic::Response<super::proto::RenderMapResponse>, tonic::Status> {
        assert_eq!(
            request.metadata().get("authorization").unwrap(),
            "Bearer worker-secret"
        );
        assert_eq!(request.into_inner().points.len(), 2);
        Ok(tonic::Response::new(super::proto::RenderMapResponse {
            png: b"\x89PNG\r\n\x1a\nworker-png".to_vec(),
            cache_hit: false,
        }))
    }
}

#[test]
fn legacy_cache_keys_are_retained() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        normalized: RenderRequest,
        key: String,
    }
    let fixtures: Vec<Fixture> = serde_json::from_str(include_str!(
        "../../../../map-renderer/testdata/legacy-requests.json"
    ))
    .unwrap();
    for fixture in fixtures {
        assert_eq!(fixture.normalized.cache_key(), fixture.key);
    }
}

#[tokio::test]
async fn cache_reuses_files_and_renews_idle_expiry() {
    let directory = tempfile::tempdir().unwrap();
    let cache = Cache::new(directory.path().into(), Duration::from_secs(60))
        .await
        .unwrap();
    let key = "a".repeat(64);
    assert!(cache.read(key.clone()).await.unwrap().is_none());
    cache
        .write(key.clone(), b"first png".to_vec())
        .await
        .unwrap();
    let path = directory.path().join(format!("{key}.png"));
    let previous = SystemTime::now() - Duration::from_secs(30);
    File::open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(previous))
        .unwrap();
    assert_eq!(
        cache.read(key.clone()).await.unwrap().unwrap(),
        b"first png"
    );
    assert!(fs::metadata(&path).unwrap().modified().unwrap() > previous);
    cache
        .write(key.clone(), b"replacement png".to_vec())
        .await
        .unwrap();
    assert_eq!(
        cache.read(key.clone()).await.unwrap().unwrap(),
        b"replacement png"
    );
    File::open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(61)))
        .unwrap();
    assert!(cache.read(key).await.unwrap().is_none());
    assert!(!path.exists());
}

#[tokio::test]
async fn pruning_removes_only_idle_renderer_files() {
    let directory = tempfile::tempdir().unwrap();
    let cache = Cache::new(directory.path().into(), Duration::from_secs(60))
        .await
        .unwrap();
    for name in [
        format!("{}.png", "a".repeat(64)),
        format!("{}.png.tmp-old", "b".repeat(64)),
        "notes.png".into(),
    ] {
        let path = directory.path().join(name);
        fs::write(&path, b"old").unwrap();
        File::open(path)
            .unwrap()
            .set_times(FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(61)))
            .unwrap();
    }
    cache
        .write("c".repeat(64), b"fresh".to_vec())
        .await
        .unwrap();
    fs::create_dir(directory.path().join("unrelated-directory")).unwrap();
    let stats = cache.prune().await.unwrap();
    assert_eq!(stats.pruned, 2);
    assert_eq!(stats.images, 1);
    assert_eq!(stats.bytes, 8);
    assert!(directory.path().join("notes.png").exists());
    assert!(directory.path().join("unrelated-directory").exists());
    assert!(Cache::new(directory.path().into(), Duration::ZERO)
        .await
        .is_err());
}

#[tokio::test]
async fn cache_write_failure_cleans_temporary_files() {
    let directory = tempfile::tempdir().unwrap();
    let cache = Cache::new(directory.path().into(), Duration::from_secs(60))
        .await
        .unwrap();
    let key = "a".repeat(64);
    fs::create_dir(directory.path().join(format!("{key}.png"))).unwrap();
    assert!(cache.write(key.clone(), b"png".to_vec()).await.is_err());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    assert!(cache.read(key).await.is_err());
    fs::remove_dir_all(directory.path()).unwrap();
    assert!(cache.prune().await.is_err());
}
