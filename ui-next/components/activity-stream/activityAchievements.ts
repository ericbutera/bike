import {
  type Activity,
  type ActivityAchievementHighlight,
} from "../../lib/queries";
import { primarySegmentAchievement } from "../../lib/segmentAchievements";

export type StreamAchievement = {
  key: string;
  label: string;
  segmentTitle: string;
  tone: "kom" | "top-10" | "pr";
  priority: number;
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

function streamAchievementPriority(achievement: StreamAchievement) {
  switch (achievement.tone) {
    case "kom":
      return 0;
    case "top-10":
      return 10 + achievement.priority;
    case "pr":
      return 20 + achievement.priority;
  }
}

function streamAchievement(
  effort: ActivityAchievementHighlight,
): StreamAchievement[] {
  const primary = primarySegmentAchievement({
    overallRank: effort.overall_rank,
    personalRank: effort.personal_rank,
  });

  if (primary) {
    if (primary.kind === "fastest") {
      return [];
    }

    return [
      {
        key: `${effort.segment_id}-${effort.effort_index}-${primary.shortLabel}`,
        label: primary.longLabel,
        segmentTitle: effort.segment_title,
        tone: primary.kind,
        priority: primary.overallRank ?? effort.personal_rank ?? 0,
      },
    ];
  }

  if (
    effort.personal_rank != null &&
    effort.personal_rank >= 2 &&
    effort.personal_rank <= 3
  ) {
    return [
      {
        key: `${effort.segment_id}-${effort.effort_index}-personal-${effort.personal_rank}`,
        label: personalBestLabel(effort.personal_rank),
        segmentTitle: effort.segment_title,
        tone: "pr",
        priority: effort.personal_rank,
      },
    ];
  }

  return [];
}

export function activityAchievements(activity: Activity) {
  return (activity.achievement_highlights ?? activity.segment_efforts ?? [])
    .flatMap((effort) => streamAchievement(effort))
    .sort((left, right) => {
      const leftPriority = streamAchievementPriority(left);
      const rightPriority = streamAchievementPriority(right);

      if (leftPriority !== rightPriority) {
        return leftPriority - rightPriority;
      }

      return left.segmentTitle.localeCompare(right.segmentTitle);
    });
}
