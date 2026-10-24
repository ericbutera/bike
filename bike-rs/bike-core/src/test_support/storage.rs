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
pub async fn matching_recording(
    objects: &ObjectStore,
    activity: &crate::entities::activities::Model,
    route_points: Vec<crate::activity_data::ActivityRoutePoint>,
) -> crate::entities::activity_recordings::Model {
    use crate::entities::activity_recordings::{Model, Recording, FORMAT_VERSION};
    let reference = objects
        .for_owner(ObjectOwner::BikeTasks)
        .transform(
            &format!("bike/tasks/recordings/{}", activity.user_id),
            &Recording {
                format_version: FORMAT_VERSION,
                context: Default::default(),
                chart_points: Vec::new(),
                route_points,
            },
        )
        .await
        .expect("valid normalized recording fixture");
    Model {
        activity_id: activity.id,
        ..super::activity_recording(&reference)
    }
}
