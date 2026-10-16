use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult, Value};
use std::collections::BTreeMap;

pub fn postgres_mock() -> MockDatabase {
    MockDatabase::new(DatabaseBackend::Postgres)
}

pub fn written() -> MockExecResult {
    MockExecResult {
        rows_affected: 1,
        ..Default::default()
    }
}

pub fn count_row(count: i64) -> BTreeMap<String, Value> {
    [("num_items".into(), count.into())].into()
}
