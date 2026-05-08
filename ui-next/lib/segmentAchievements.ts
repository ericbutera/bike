export type SegmentAchievementKind = "kom" | "top-10" | "pr" | "fastest";

export type SegmentAchievement = {
  kind: SegmentAchievementKind;
  shortLabel: string;
  longLabel: string;
  overallRank?: number;
};

export function primarySegmentAchievement({
  overallRank,
  personalRank,
  isFastestOfDay = false,
}: {
  overallRank?: number | null;
  personalRank?: number | null;
  isFastestOfDay?: boolean;
}): SegmentAchievement | null {
  if (overallRank === 1) {
    return {
      kind: "kom",
      shortLabel: "KOM",
      longLabel: "KOM",
      overallRank: 1,
    };
  }

  if (overallRank != null && overallRank >= 2 && overallRank <= 10) {
    return {
      kind: "top-10",
      shortLabel: `Top ${overallRank}`,
      longLabel: `Top ${overallRank}`,
      overallRank,
    };
  }

  if (personalRank === 1) {
    return {
      kind: "pr",
      shortLabel: "PR",
      longLabel: "PR",
    };
  }

  if (isFastestOfDay) {
    return {
      kind: "fastest",
      shortLabel: "Fastest",
      longLabel: "Fastest run today",
    };
  }

  return null;
}
