use super::*;
use crate::test_support;
use ::object_store::{ObjectStoreExt, PutPayload};

#[tokio::test]
async fn repeated_and_concurrent_writes_return_one_immutable_reference() {
    let (store, backend) = test_support::object_storage(ObjectOwner::BikeTasks);
    let prefix = "bike/tasks/7/reprocess/42/source";
    let bytes = b"exact original input\0\xff";
    let (first, second) = tokio::join!(store.write(prefix, bytes), store.write(prefix, bytes));
    let first = first.unwrap();
    assert_eq!(first, second.unwrap());
    assert_eq!(first, store.write(prefix, bytes).await.unwrap());
    let restarted = ObjectStore::new(backend, ObjectOwner::BikeTasks);
    assert_eq!(restarted.read(&first).await.unwrap(), bytes);
}

#[tokio::test]
async fn corrupt_missing_or_misreported_objects_cannot_be_reused() {
    let (store, backend) = test_support::object_storage(ObjectOwner::BikeTasks);
    let object = store.write("bike/tasks/7/source", b"original").await.unwrap();
    let wrong_size = ObjectRef {
        bytes: object.bytes + 1,
        ..object.clone()
    };
    assert_eq!(store.read(&wrong_size).await.unwrap_err().status.as_u16(), 422);
    let key = ObjectPath::parse(&object.path).unwrap();
    backend.put(&key, PutPayload::from_static(b"corrupt")).await.unwrap();
    assert_eq!(store.read(&object).await.unwrap_err().status.as_u16(), 422);
    assert!(store.write("bike/tasks/7/source", b"original").await.is_err());
    backend.delete(&key).await.unwrap();
    assert!(store.read(&object).await.is_err());
}

#[test]
fn backend_failures_remain_transient() {
    let error = storage_error(::object_store::Error::Generic {
        store: "fixture",
        source: std::io::Error::new(std::io::ErrorKind::TimedOut, "temporary outage").into(),
    });
    assert!(error.status.is_server_error());
}

#[tokio::test]
async fn malformed_retained_json_is_terminal_after_integrity_verification() {
    let (store, _) = test_support::object_storage(ObjectOwner::BikeTasks);
    let object = store.write("bike/tasks/7/source", b"malformed json").await.unwrap();
    assert_eq!(
        store
            .read_json::<serde_json::Value>(&object)
            .await
            .unwrap_err()
            .status
            .as_u16(),
        422
    );
    assert_eq!(
        store
            .load_json::<serde_json::Value>(&object.path)
            .await
            .unwrap_err()
            .status
            .as_u16(),
        422
    );
}

#[tokio::test]
async fn owners_cannot_read_write_or_remove_each_others_keys() {
    let (bike, backend) = test_support::object_storage(ObjectOwner::BikeTasks);
    let strava = ObjectStore::new(backend, ObjectOwner::Strava);
    let object = strava.write("strava/7/raw", b"provider").await.unwrap();
    assert!(bike.read(&object).await.is_err());
    assert!(bike.remove(&object.path).await.is_err());
    let object = bike.write("bike/tasks/7/raw", b"upload").await.unwrap();
    assert!(strava.read(&object).await.is_err());
    for key in [
        "",
        "/bike/tasks/7",
        "../bike/tasks/7",
        "bike/tasks/../strava",
        "bike/tasks-other/7",
        "strava/7",
    ] {
        assert!(bike.write(key, b"blocked").await.is_err(), "{key}");
    }
}

#[tokio::test]
async fn retained_imports_keep_their_existing_keys_and_namespace() {
    let (imports, backend) = test_support::object_storage(ObjectOwner::ActivityImports);
    let artifact = test_support::reprocess_artifact();
    let reference = test_support::reprocess_execution().command.source.object;
    backend
        .put(
            &ObjectPath::parse(&artifact.storage_path).unwrap(),
            test_support::REPROCESS_SOURCE.into(),
        )
        .await
        .unwrap();
    assert_eq!(imports.read(&reference).await.unwrap(), test_support::REPROCESS_SOURCE);
    for owner in [ObjectOwner::BikeTasks, ObjectOwner::Strava] {
        let other = ObjectStore::new(backend.clone(), owner);
        assert_eq!(other.read(&reference).await.unwrap_err().status.as_u16(), 400);
        let foreign = other
            .write(&format!("{}/7/source", owner.prefix()), b"foreign")
            .await
            .unwrap();
        assert_eq!(imports.read(&foreign).await.unwrap_err().status.as_u16(), 400);
    }
    for key in [
        "/activity-imports/source",
        "activity-imports/../strava/source",
        "activity-imports-other/source",
    ] {
        let invalid = ObjectRef {
            path: key.into(),
            ..reference.clone()
        };
        assert_eq!(imports.read(&invalid).await.unwrap_err().status.as_u16(), 400);
    }
}

#[tokio::test]
async fn references_keep_the_existing_wire_format_and_round_trip_json() {
    let (store, _) = test_support::object_storage(ObjectOwner::Strava);
    let value = serde_json::json!({"activity_id": 42});
    let object = store.transform("strava/7/parsed", &value).await.unwrap();
    assert_eq!(store.read_json::<serde_json::Value>(&object).await.unwrap(), value);
    let serialized = serde_json::json!({ "path": object.path, "sha256": object.sha256, "bytes": object.bytes });
    assert_eq!(serde_json::to_value(&object).unwrap(), serialized);
    assert_eq!(serde_json::from_value::<ObjectRef>(serialized).unwrap(), object);
}

#[tokio::test]
async fn named_references_are_idempotent_and_cannot_change_the_retained_source() {
    let (store, _) = test_support::object_storage(ObjectOwner::BikeTasks);
    let key = "bike/tasks/7/import-18";
    assert!(store.reference(key).await.unwrap().is_none());
    let first = store.write(key, b"source one").await.unwrap();
    store.save_reference(key, &first).await.unwrap();
    store.save_reference(key, &first).await.unwrap();
    let different = store.write(key, b"source two").await.unwrap();
    assert!(store.save_reference(key, &different).await.is_err());
    assert_eq!(store.reference(key).await.unwrap(), Some(first));
}

#[tokio::test]
async fn concurrent_named_descriptors_return_the_winning_value() {
    let (store, _) = test_support::object_storage(ObjectOwner::BikeTasks);
    let key = "bike/tasks/7/import-18/manifest.json";
    let first = serde_json::json!({"revision": 1});
    let second = serde_json::json!({"revision": 2});
    let (first, second) = tokio::join!(store.create_json(key, &first), store.create_json(key, &second));
    assert_eq!(first.unwrap(), second.unwrap());
}

#[tokio::test]
async fn explicit_domain_deletion_is_idempotent() {
    let (store, _) = test_support::object_storage(ObjectOwner::BikeTasks);
    let object = store.write("bike/tasks/7/source", b"original").await.unwrap();
    store.remove(&object.path).await.unwrap();
    store.remove(&object.path).await.unwrap();
    assert!(store.read(&object).await.is_err());
}

#[tokio::test]
async fn filesystem_adapter_creates_its_root_and_survives_a_new_instance() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("objects");
    let store = ObjectStore::filesystem(&root, ObjectOwner::BikeTasks);
    let object = store.write("bike/tasks/7/source", b"retained").await.unwrap();
    let restarted = ObjectStore::filesystem(root, ObjectOwner::BikeTasks);
    assert_eq!(restarted.read(&object).await.unwrap(), b"retained");
}

#[tokio::test]
async fn filesystem_scratch_uses_the_writable_storage_root_and_is_removed_on_drop() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("objects");
    let store = ObjectStore::filesystem(&root, ObjectOwner::Archives);
    let key = "archives/20.zip";
    store.write_key(key, b"retained archive").await.unwrap();
    let temporary = store.materialize(key).await.unwrap();
    let path = temporary.path().to_owned();
    assert!(path.starts_with(root.join(".scratch")));
    assert_eq!(tokio::fs::read(&path).await.unwrap(), b"retained archive");
    drop(temporary);
    assert!(!path.exists());
    assert!(store.contains(key).await.unwrap());
}

#[tokio::test]
async fn named_import_keys_preserve_bytes_and_reject_conflicting_rewrites() {
    let (store, _) = test_support::object_storage(ObjectOwner::ActivityImports);
    let key = "activity-imports/user/2026/10/source.gpx";
    let first = store.write_key(key, b"source bytes").await.unwrap();
    assert_eq!(first.path, key);
    assert_eq!(store.write_key(key, b"source bytes").await.unwrap(), first);
    assert!(store.write_key(key, b"different bytes").await.is_err());
    assert_eq!(store.read(&first).await.unwrap(), b"source bytes");
}

#[tokio::test]
async fn large_archives_round_trip_through_an_injected_backend_and_disposable_scratch() {
    let (imports, backend) = test_support::object_storage(ObjectOwner::ActivityImports);
    let archives = imports.for_owner(ObjectOwner::Archives);
    let source = tempfile::NamedTempFile::new().unwrap();
    let bytes = vec![42; 11 * 1024 * 1024];
    tokio::fs::write(source.path(), &bytes).await.unwrap();
    let key = "archives/20.zip";
    assert!(!archives.contains(key).await.unwrap());
    archives.retain_file(key, source.path()).await.unwrap();
    drop(source);
    let restarted = ObjectStore::new(backend, ObjectOwner::Archives);
    assert!(restarted.contains(key).await.unwrap());
    let scratch = restarted.materialize(key).await.unwrap();
    let path = scratch.path().to_owned();
    assert_eq!(tokio::fs::read(&path).await.unwrap(), bytes);
    drop(scratch);
    assert!(!path.exists());
    assert!(imports.materialize(key).await.is_err());
    assert!(imports.contains(key).await.is_err());
    assert!(archives.materialize("archives/missing.zip").await.is_err());
}

#[tokio::test]
async fn file_retention_rejects_foreign_keys_and_aborts_failed_reads() {
    let (store, _) = test_support::object_storage(ObjectOwner::Archives);
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.zip");
    assert!(store.retain_file("strava/source.zip", &missing).await.is_err());
    assert!(store.retain_file("archives/20.zip", &missing).await.is_err());
    // Opening a directory succeeds on Linux, but reading fails after the
    // multipart upload starts. No partial object may be published.
    assert!(store.retain_file("archives/20.zip", directory.path()).await.is_err());
    assert!(!store.contains("archives/20.zip").await.unwrap());
}
