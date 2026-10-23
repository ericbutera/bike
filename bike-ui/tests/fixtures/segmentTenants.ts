import type { components } from "../../lib/openapi/react-query/api";
import { overallAchievement, personalAchievement } from "./activitySegments";

type TenantUser = components["schemas"]["UserResponse"] & { id: number };

export function segmentTenants() {
  const [creator, contributor, spectator] = [7, 8, 9].map((id): TenantUser => ({
    id,
    pid: `00000000-0000-4000-8000-${String(id).padStart(12, "0")}`,
    email: `rider-${id}@example.test`,
    name: `Rider ${id}`,
    disabled: false,
    verified: true,
    is_admin: false,
  }));
  const personal = [
    {
      userId: 7,
      segmentId: 1,
      row: personalAchievement({ effort_id: 1, duration_seconds: 100 }),
    },
    {
      userId: 7,
      segmentId: 1,
      row: personalAchievement({
        position: 2,
        effort_id: 2,
        duration_seconds: 200,
      }),
    },
    {
      userId: 8,
      segmentId: 1,
      row: personalAchievement({ effort_id: 3, duration_seconds: 300 }),
    },
    {
      userId: 8,
      segmentId: 2,
      row: personalAchievement({ effort_id: 4, duration_seconds: 50 }),
    },
    {
      userId: 7,
      segmentId: 2,
      row: personalAchievement({ effort_id: 5, duration_seconds: 150 }),
    },
  ];
  const overall = new Map([
    [
      1,
      [
        overallAchievement({
          user_id: 7,
          rider_name: creator.name,
          effort_id: 1,
          duration_seconds: 100,
        }),
        overallAchievement({
          position: 2,
          user_id: 8,
          rider_name: contributor.name,
          effort_id: 3,
          duration_seconds: 300,
        }),
      ],
    ],
    [
      2,
      [
        overallAchievement({
          user_id: 8,
          rider_name: contributor.name,
          effort_id: 4,
          duration_seconds: 50,
        }),
        overallAchievement({
          position: 2,
          user_id: 7,
          rider_name: creator.name,
          effort_id: 5,
          duration_seconds: 150,
        }),
      ],
    ],
  ]);
  return {
    creator,
    contributor,
    spectator,
    personalFor: (user: TenantUser | null, segmentId: number) =>
      personal
        .filter(
          (entry) => entry.userId === user?.id && entry.segmentId === segmentId,
        )
        .map((entry) => entry.row),
    overallFor: (segmentId: number) => overall.get(segmentId) ?? [],
  };
}
