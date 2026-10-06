//! Recording evidence is independent of the transport, sport and regenerated GPS data.
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub(crate) const INDOOR_RECORDING_TOKENS: &[&str] = &[
    "indoor",
    "indoorcycling",
    "indoortrainerride",
    "indoortrainer",
    "spin",
    "trainer",
    "trainerroad",
    "trainerday",
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecordingEnvironment {
    #[default]
    Unknown,
    Outdoor,
    Indoor,
    Virtual,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RecordingContext {
    #[serde(default)]
    pub environment: RecordingEnvironment,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<RecordingEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RecordingEvidence {
    pub field: String,
    pub value: String,
}

impl RecordingContext {
    pub fn excluded(&self) -> bool {
        matches!(
            self.environment,
            RecordingEnvironment::Indoor | RecordingEnvironment::Virtual
        )
    }

    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Absent flags, generic exports and title edits cannot erase positive evidence.
    pub fn merge(&mut self, other: Self) {
        if other.environment.rank() > self.environment.rank() {
            self.environment = other.environment;
        }
        if self.platform.is_none() || other.platform.as_deref() == Some("zwift") {
            self.platform = other.platform;
        }
        for evidence in other.evidence {
            if !self.evidence.contains(&evidence) {
                self.evidence.push(evidence);
            }
        }
    }

    pub fn observe(&mut self, field: &str, value: &str) {
        let token: String = value
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .flat_map(char::to_lowercase)
            .collect();
        let environment = if token.contains("zwift") || token.contains("virtual") {
            RecordingEnvironment::Virtual
        } else if INDOOR_RECORDING_TOKENS.contains(&token.as_str()) {
            RecordingEnvironment::Indoor
        } else if token == "outdoor" {
            RecordingEnvironment::Outdoor
        } else {
            return;
        };
        self.merge(Self {
            environment,
            platform: token.contains("zwift").then(|| "zwift".into()),
            evidence: vec![RecordingEvidence {
                field: field.into(),
                value: value.into(),
            }],
        });
    }

    pub fn trainer(&mut self, field: &str, enabled: bool) {
        if enabled {
            self.merge(Self {
                environment: RecordingEnvironment::Indoor,
                evidence: vec![RecordingEvidence {
                    field: field.into(),
                    value: "true".into(),
                }],
                ..Self::default()
            });
        }
    }

    pub fn observe_recorder(&mut self, field: &str, value: &str) {
        self.observe(field, value);
        let token: String = value
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .flat_map(char::to_lowercase)
            .collect();
        let platform = match token.as_str() {
            "garmin" | "garminconnect" => "garmin",
            "wahoo" | "wahoofitness" => "wahoo",
            "trainerroad" => "trainerroad",
            "trainerday" => "trainerday",
            _ => return,
        };
        self.merge(Self {
            platform: Some(platform.into()),
            evidence: vec![RecordingEvidence {
                field: field.into(),
                value: value.into(),
            }],
            ..Self::default()
        });
    }

    pub fn legacy(sport: &str, source: &str, title: &str) -> Self {
        let mut context = Self::default();
        context.observe("sport", sport);
        context.observe("import_source", source);
        if title.to_ascii_lowercase().contains("zwift") {
            context.observe("title", "zwift");
        }
        context
    }

    pub fn from_xml(document: &roxmltree::Document<'_>) -> Self {
        let mut context = Self::default();
        for node in document.descendants().filter(roxmltree::Node::is_element) {
            let name = node.tag_name().name();
            match name {
                "gpx" => {
                    context.observe_recorder("gpx.creator", node.attribute("creator").unwrap_or(""))
                }
                "Activity" => context.observe("tcx.Sport", node.attribute("Sport").unwrap_or("")),
                "Creator" | "Author" => {
                    for value in node.descendants().filter_map(|child| child.text()) {
                        context.observe_recorder(name, value);
                    }
                }
                "type"
                | "SubSport"
                | "RecordingEnvironment"
                | "RecordingPlatform"
                | "tag"
                | "Tag" => {
                    context.observe(name, node.text().unwrap_or(""));
                }
                "Trainer" | "Indoor" => context.trainer(
                    name,
                    node.text()
                        .is_some_and(|value| value.trim().eq_ignore_ascii_case("true")),
                ),
                _ => {}
            }
        }
        context
    }
}

impl RecordingEnvironment {
    fn rank(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::Outdoor => 1,
            Self::Indoor => 2,
            Self::Virtual => 3,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Outdoor => "outdoor",
            Self::Indoor => "indoor",
            Self::Virtual => "virtual",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_exports_and_false_trainer_flags_cannot_clear_virtual_evidence() {
        let mut context = RecordingContext::default();
        context.observe("file_id.manufacturer", "zwift");
        context.merge(RecordingContext::legacy(
            "Ride",
            "garmin_archive",
            "Outdoor ride",
        ));
        context.trainer("strava.trainer", false);
        assert_eq!(context.environment, RecordingEnvironment::Virtual);
        assert_eq!(context.platform.as_deref(), Some("zwift"));
    }

    #[test]
    fn real_activity_and_sensor_metadata_do_not_imply_virtual() {
        let context =
            RecordingContext::legacy("mountain_bike", "archive_url_import", "Workout outdoors");
        assert!(!context.excluded());
        let document = roxmltree::Document::parse(
            "<gpx creator='Garmin'><trk><type>cycling</type></trk></gpx>",
        )
        .unwrap();
        assert!(!RecordingContext::from_xml(&document).excluded());
    }

    #[test]
    fn explicit_indoor_tags_exclude_generic_ride_exports() {
        for tag in ["trainer", "indoor"] {
            let xml = format!("<gpx creator='Garmin'><trk><type>Ride</type><extensions><tags><tag>{tag}</tag></tags></extensions></trk></gpx>");
            let document = roxmltree::Document::parse(&xml).unwrap();
            assert_eq!(
                RecordingContext::from_xml(&document).environment,
                RecordingEnvironment::Indoor
            );
        }
    }
}
