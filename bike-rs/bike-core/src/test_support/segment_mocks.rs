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

impl SharedSegments {
    pub fn personal_read_mock(&self, reader: &users::Model, segment: &segments::Model) -> MockDatabase {
        segment_access_mock(reader, segment).append_query_results([self.personal_standings_for(reader.id, segment.id)])
    }

    pub fn comparison_read_mock(&self, reader: &users::Model, segment: &segments::Model) -> MockDatabase {
        let efforts: Vec<_> = self
            .efforts_for(segment.id)
            .into_iter()
            .filter(|row| row.user_id == reader.id)
            .collect();
        let empty = efforts.is_empty();
        let mock = segment_access_mock(reader, segment).append_query_results([efforts]);
        if empty {
            return mock;
        }
        let activities: Vec<_> = self
            .activities
            .iter()
            .filter(|row| row.user_id == reader.id)
            .cloned()
            .collect();
        mock.append_query_results([activities])
            .append_query_results([vec![reader.clone()]])
    }
}
