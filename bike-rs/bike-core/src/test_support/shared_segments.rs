//! A shared segment graph for access, ranking, and segment-isolation scenarios.
use super::{activity, customer, overall_achievement, segment, segment_effort};
use crate::auth::entities::users;
use crate::entities::{activities, segment_efforts, segment_overall_achievements, segments};

pub struct SharedSegments {
    pub creator: users::Model,
    pub contributor: users::Model,
    pub spectator: users::Model,
    pub activities: [activities::Model; 2],
    pub segment: segments::Model,
    pub other_segment: segments::Model,
    pub efforts: Vec<segment_efforts::Model>,
    pub standings: Vec<segment_overall_achievements::Model>,
}

impl SharedSegments {
    pub fn users(&self) -> Vec<users::Model> {
        vec![self.creator.clone(), self.contributor.clone(), self.spectator.clone()]
    }

    pub fn efforts_for(&self, segment_id: i32) -> Vec<segment_efforts::Model> {
        self.efforts
            .iter()
            .filter(|row| row.segment_id == segment_id)
            .cloned()
            .collect()
    }

    pub fn standings_for(&self, segment_id: i32) -> Vec<segment_overall_achievements::Model> {
        self.standings
            .iter()
            .filter(|row| row.segment_id == segment_id)
            .cloned()
            .collect()
    }
}

pub fn shared_segments() -> SharedSegments {
    let [creator, contributor, spectator] = [7, 8, 9].map(rider);
    let activities = [
        activity(),
        activities::Model {
            id: 43,
            user_id: contributor.id,
            ..activity()
        },
    ];
    let segment = segment();
    let other_segment = segments::Model {
        id: 2,
        user_id: contributor.id,
        title: "Other segment".into(),
        ..segment.clone()
    };
    let efforts = vec![
        effort(1, &activities[0], segment.id, 100),
        segment_efforts::Model {
            effort_index: 2,
            start_route_point_index: 2,
            end_route_point_index: 3,
            start_elapsed_seconds: 200,
            end_elapsed_seconds: 400,
            ..effort(2, &activities[0], segment.id, 200)
        },
        effort(3, &activities[1], segment.id, 300),
        segment_efforts::Model {
            start_route_point_index: 2,
            end_route_point_index: 3,
            start_elapsed_seconds: 400,
            end_elapsed_seconds: 450,
            ..effort(4, &activities[1], other_segment.id, 50)
        },
        segment_efforts::Model {
            start_route_point_index: 4,
            end_route_point_index: 5,
            start_elapsed_seconds: 500,
            end_elapsed_seconds: 650,
            ..effort(5, &activities[0], other_segment.id, 150)
        },
    ];
    // Explicit expected results: never call the ranking code under test here.
    let standings = vec![
        segment_overall_achievements::Model {
            duration_seconds: 100,
            ..overall_achievement()
        },
        segment_overall_achievements::Model {
            position: 2,
            user_id: contributor.id,
            effort_id: 3,
            duration_seconds: 300,
            ..overall_achievement()
        },
        segment_overall_achievements::Model {
            segment_id: other_segment.id,
            user_id: contributor.id,
            effort_id: 4,
            duration_seconds: 50,
            ..overall_achievement()
        },
        segment_overall_achievements::Model {
            segment_id: other_segment.id,
            position: 2,
            effort_id: 5,
            duration_seconds: 150,
            ..overall_achievement()
        },
    ];
    SharedSegments {
        creator,
        contributor,
        spectator,
        activities,
        segment,
        other_segment,
        efforts,
        standings,
    }
}

fn rider(id: i32) -> users::Model {
    users::Model {
        id,
        pid: uuid::Uuid::from_u128(id.try_into().unwrap()),
        email: format!("rider-{id}@example.test"),
        api_key: format!("rider-{id}-key"),
        name: format!("Rider {id}"),
        ..customer()
    }
}

fn effort(id: i32, activity: &activities::Model, segment_id: i32, duration_seconds: i32) -> segment_efforts::Model {
    segment_efforts::Model {
        id,
        activity_id: activity.id,
        user_id: activity.user_id,
        segment_id,
        duration_seconds,
        end_elapsed_seconds: duration_seconds,
        ..segment_effort()
    }
}

pub fn shared_segment_efforts() -> Vec<segment_efforts::Model> {
    let scenario = shared_segments();
    scenario.efforts_for(scenario.segment.id)
}

/// One effort per athlete for leaderboard cutoff scenarios.
pub fn distinct_athlete_efforts(count: i32) -> Vec<segment_efforts::Model> {
    (1..=count)
        .map(|id| segment_efforts::Model {
            id,
            user_id: id,
            duration_seconds: 100 + id,
            ..segment_effort()
        })
        .collect()
}

impl SharedSegments {
    pub fn personal_standings_for(
        &self,
        user_id: i32,
        segment_id: i32,
    ) -> Vec<crate::entities::segment_personal_achievements::Model> {
        use crate::entities::segment_personal_achievements::Model;
        let rows = vec![
            Model {
                duration_seconds: 100,
                ..super::personal_achievement()
            },
            Model {
                position: 2,
                effort_id: 2,
                duration_seconds: 200,
                ..super::personal_achievement()
            },
            Model {
                user_id: self.contributor.id,
                effort_id: 3,
                duration_seconds: 300,
                ..super::personal_achievement()
            },
            Model {
                segment_id: self.other_segment.id,
                user_id: self.contributor.id,
                effort_id: 4,
                duration_seconds: 50,
                ..super::personal_achievement()
            },
            Model {
                segment_id: self.other_segment.id,
                effort_id: 5,
                duration_seconds: 150,
                ..super::personal_achievement()
            },
        ];
        rows.into_iter()
            .filter(|row| row.user_id == user_id && row.segment_id == segment_id)
            .collect()
    }
}
