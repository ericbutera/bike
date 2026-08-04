import {
  faCrown,
  faMedal,
  faTrophy,
} from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { type StreamAchievement } from "./activityAchievements";

function streamAchievementBadgeClassName(tone: StreamAchievement["tone"]) {
  switch (tone) {
    case "kom":
      return "border-warning/40 bg-warning/10 text-warning-content";
    case "top-10":
      return "border-info/40 bg-info/10 text-info-content";
    case "pr":
      return "border-primary/40 bg-primary/10 text-primary-content";
  }
}

function streamAchievementIcon(tone: StreamAchievement["tone"]) {
  switch (tone) {
    case "kom":
      return faCrown;
    case "top-10":
      return faTrophy;
    case "pr":
      return faMedal;
  }
}

export default function ActivityAchievementChip({
  achievement,
}: {
  achievement: StreamAchievement;
}) {
  return (
    <span
      className={`inline-flex max-w-full items-center gap-1.5 rounded-full border px-3 py-1 text-xs font-medium ${streamAchievementBadgeClassName(achievement.tone)}`}
      title={`${achievement.segmentTitle} · ${achievement.label}`}
    >
      <FontAwesomeIcon
        icon={streamAchievementIcon(achievement.tone)}
        className="h-3 w-3 shrink-0"
      />
      <span className="shrink-0">{achievement.label}</span>
      <span className="truncate">{achievement.segmentTitle}</span>
    </span>
  );
}
