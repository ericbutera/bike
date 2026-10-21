// Real route geometry, fictional people and timing. IDs/dates stay stable so
// automated assertions and a reviewer in staging can refer to the same rides.
export const competitionRiders = [
  1,
  ...Array.from({ length: 14 }, (_, i) => 101 + i),
];
export const competitionSegments = [4101, 4102, 4103, 4104, 4105];
export const competitionCases = {
  earnedTopTen: 31001,
  displacedTopTen: 31002,
  newKom: 31003,
  yearlyBest: 31004,
  repeatedAttempts: 31005,
  slowerYearlyRival: 31006,
};

function ride(id, userId, route, startedAt, pace, repeats = 1) {
  return { id, user_id: userId, route, started_at: startedAt, pace, repeats };
}

export function competitionRides() {
  const baseline = competitionRiders.flatMap((userId, index) => {
    const pace =
      index === 0
        ? 1.3
        : index <= 9
          ? 0.55 + (index - 1) * 0.04
          : 0.68 + index * 0.04;
    return [
      ride(30000 + index * 2, userId, "city", "2025-05-31T08:00:00Z", pace),
      ride(30001 + index * 2, userId, "unmarked", "2025-05-31T16:00:00Z", pace),
    ];
  });
  const personalHistory = Array.from({ length: 10 }, (_, index) =>
    ride(
      31100 + index,
      1,
      "city",
      `2024-${String(index + 1).padStart(2, "0")}-15T08:00:00Z`,
      1.8 - index * 0.04,
    ),
  );
  return [
    ...personalHistory,
    ...baseline,
    ride(competitionCases.earnedTopTen, 1, "city", "2025-06-01T08:00:00Z", 1),
    ride(
      competitionCases.displacedTopTen,
      110,
      "city",
      "2025-06-01T16:00:00Z",
      0.95,
    ),
    ride(competitionCases.newKom, 102, "city", "2025-06-02T08:00:00Z", 0.5),
    ride(competitionCases.yearlyBest, 1, "city", "2026-06-01T08:00:00Z", 1.1),
    ride(
      competitionCases.repeatedAttempts,
      1,
      "city",
      "2026-07-06T08:00:00Z",
      1.2,
      3,
    ),
    ride(
      competitionCases.slowerYearlyRival,
      102,
      "city",
      "2026-06-01T16:00:00Z",
      1.25,
    ),
  ];
}
