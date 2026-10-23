import { type Activity, type ActivityEarnedAward } from "../../lib/queries";
import { primarySegmentAchievement } from "../../lib/segmentAchievements";

export type StreamAchievement = {
  key: string;
  label: string;
  segmentTitle: string;
  tone: "kom" | "top-10" | "pr";
};

function personalBestLabel(rank: number) {
  if (rank === 1) {
    return "PR";
  }

  if (rank === 2) {
    return "2nd best";
  }

  return "3rd best";
}

function streamAchievement(award: ActivityEarnedAward): StreamAchievement[] {
  const primary = primarySegmentAchievement({
    overallRank: award.overall_rank,
    personalRank: award.personal_rank,
  });

  if (primary) {
    if (primary.kind === "fastest") {
      return [];
    }

    return [
      {
        key: `${award.activity_id}-${award.position}`,
        label: primary.longLabel,
        segmentTitle: award.segment_title,
        tone: primary.kind,
      },
    ];
  }

  if (
    award.personal_rank != null &&
    award.personal_rank >= 2 &&
    award.personal_rank <= 3
  ) {
    return [
      {
        key: `${award.activity_id}-${award.position}`,
        label: personalBestLabel(award.personal_rank),
        segmentTitle: award.segment_title,
        tone: "pr",
      },
    ];
  }

  return [];
}

export function activityAchievements(activity: Activity) {
  return activity.earned_awards.flatMap((award) => streamAchievement(award));
}
