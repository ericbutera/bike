//! PostgreSQL mock setup shared by segment domain and API tests.
use super::{postgres_mock, segment, shared_segments, SharedSegments};
use crate::auth::entities::users;
use crate::entities::{segment_efforts, segments};
use sea_orm::{DatabaseConnection, MockDatabase};

pub fn segment_access_mock(reader: &users::Model, segment: &segments::Model) -> MockDatabase {
    postgres_mock()
        .append_query_results([vec![reader.clone()]])
        .append_query_results([vec![segment.clone()]])
}

pub fn segment_rebuild_mock(efforts: Vec<segment_efforts::Model>) -> MockDatabase {
    postgres_mock()
        .append_query_results([vec![segment()]])
        .append_query_results([efforts])
}

pub fn saved_overall_achievements_db() -> DatabaseConnection {
    let scenario = shared_segments();
    postgres_mock()
        .append_query_results([scenario.standings_for(scenario.segment.id)])
        .into_connection()
}

impl SharedSegments {
    pub fn overall_read_mock(&self, reader: &users::Model, segment: &segments::Model) -> MockDatabase {
        segment_access_mock(reader, segment)
            .append_query_results([self.standings_for(segment.id)])
            .append_query_results([self.users()])
    }
}
