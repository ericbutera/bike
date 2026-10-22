//! Shared recording bytes; paths are owned here rather than at each test call site.

pub const ACTIVITY_FIT: &[u8] = include_bytes!("../../../tests/fixtures/activity.fit");
pub const UNMARKED_01_FIT: &[u8] =
    include_bytes!("../../../tests/fixtures/segment_matching/activities/unmarked_01.fit");

pub const OUTDOOR_FIT: &[u8] = include_bytes!("../../../tests/fixtures/recording/outdoor.fit");
pub const OUTDOOR_ACCESSORY_FIT: &[u8] = include_bytes!("../../../tests/fixtures/recording/outdoor-accessory.fit");
pub const OUTDOOR_TCX: &[u8] = include_bytes!("../../../tests/fixtures/recording/outdoor.tcx");
pub const RETIRED_COPY_TCX: &[u8] = include_bytes!("../../../tests/fixtures/recording/retired-copy.tcx");
pub const VIRTUAL_FIT: &[u8] = include_bytes!("../../../tests/fixtures/recording/virtual.fit");
pub const VIRTUAL_GENERIC_FIT: &[u8] = include_bytes!("../../../tests/fixtures/recording/virtual-generic.fit");
pub const INDOOR_FIT: &[u8] = include_bytes!("../../../tests/fixtures/recording/indoor.fit");
pub const GPS_GAP_FIT: &[u8] = include_bytes!("../../../tests/fixtures/recording/gps-gap.fit");
