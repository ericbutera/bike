use crate::object_store::{ObjectOwner, ObjectStore};
use ::object_store::memory::InMemory;
use std::sync::Arc;

pub fn object_storage(owner: ObjectOwner) -> (ObjectStore, Arc<InMemory>) {
    let backend = Arc::new(InMemory::new());
    (ObjectStore::new(backend.clone(), owner), backend)
}

pub fn activity_recording(reference: &crate::object_store::ObjectRef) -> crate::entities::activity_recordings::Model {
    crate::entities::activity_recordings::Model {
        activity_id: super::activity().id,
        format_version: crate::entities::activity_recordings::FORMAT_VERSION,
        object_path: reference.path.clone(),
        object_sha256: reference.sha256.clone(),
        object_bytes: reference.bytes as i64,
    }
}
