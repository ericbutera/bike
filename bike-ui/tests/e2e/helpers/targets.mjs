export const targets = [
  {
    name: "rust",
    url: process.env.BIKE_UI_URL ?? "http://localhost:3001",
  },
];

export const activityId = process.env.BIKE_TEST_ACTIVITY_ID ?? "1109";
export const segmentId = process.env.BIKE_TEST_SEGMENT_ID ?? "5";
export const raceEffortIds =
  process.env.BIKE_TEST_RACE_EFFORT_IDS ?? "5895,5912";
export const expectedClimbCount = Number(
  process.env.BIKE_TEST_CLIMB_COUNT ?? "0",
);
export const expectedEffortCount = Number(
  process.env.BIKE_TEST_EFFORT_COUNT ?? "2",
);
export const expectZoneDistribution =
  process.env.BIKE_TEST_ZONE_DISTRIBUTION === "true";
export const snapshotSet = process.env.BIKE_TEST_SNAPSHOT_SET ?? "rust";
