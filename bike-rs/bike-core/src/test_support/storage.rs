use crate::object_store::{ObjectOwner, ObjectStore};
use ::object_store::memory::InMemory;
use std::sync::Arc;

pub fn object_storage(owner: ObjectOwner) -> (ObjectStore, Arc<InMemory>) {
    let backend = Arc::new(InMemory::new());
    (ObjectStore::new(backend.clone(), owner), backend)
}
