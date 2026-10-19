import type { IconDefinition } from "@fortawesome/fontawesome-svg-core";
import {
  faBicycle,
  faCircleQuestion,
  faDumbbell,
  faPersonBiking,
  faPersonHiking,
  faPersonRunning,
  faPersonSwimming,
  faPersonWalking,
} from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import {
  ACTIVITY_SPORT_OPTIONS,
  type ActivitySport,
} from "../lib/activitySports";
import OffRoadBikeIcon from "./icons/OffRoadBikeIcon";

const sportIcons = {
  run: faPersonRunning,
  walk: faPersonWalking,
  hike: faPersonHiking,
  mountain_bike: null,
  indoor_trainer_ride: faPersonBiking,
  road_ride: faBicycle,
  swim: faPersonSwimming,
  workout: faDumbbell,
} satisfies Record<ActivitySport, IconDefinition | null>;

// Stored/provider spellings affect presentation only; filters use the API enum.
const sportAliases: Record<string, ActivitySport | "fat_bike"> = {
  run: "run",
  running: "run",
  virtualrun: "run",
  trailrun: "run",
  walk: "walk",
  walking: "walk",
  hike: "hike",
  hiking: "hike",
  mountainbike: "mountain_bike",
  mountainbiking: "mountain_bike",
  mountainbikeride: "mountain_bike",
  mtb: "mountain_bike",
  mtbride: "mountain_bike",
  indoortrainerride: "indoor_trainer_ride",
  indoortrainer: "indoor_trainer_ride",
  indoorcycling: "indoor_trainer_ride",
  virtualride: "indoor_trainer_ride",
  roadride: "road_ride",
  roadcycling: "road_ride",
  ride: "road_ride",
  cycling: "road_ride",
  biking: "road_ride",
  bike: "road_ride",
  gravelride: "road_ride",
  ebikeride: "road_ride",
  emountainbikeride: "road_ride",
  velomobile: "road_ride",
  handcycle: "road_ride",
  fatbike: "fat_bike",
  fatbiking: "fat_bike",
  fatbikeride: "fat_bike",
  fattirebike: "fat_bike",
  fattirebiking: "fat_bike",
  swim: "swim",
  swimming: "swim",
  workout: "workout",
};

export function resolveActivitySport(sport: string) {
  const token = sport
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]/g, "");
  return sportAliases[token];
}

export default function ActivitySportIcon({ sport }: { sport: string }) {
  const normalizedSport = resolveActivitySport(sport);
  const label =
    normalizedSport === "fat_bike"
      ? "Fat tire biking"
      : (ACTIVITY_SPORT_OPTIONS.find(({ value }) => value === normalizedSport)
          ?.label ?? readableSportLabel(sport));
  const customBike =
    normalizedSport === "mountain_bike" || normalizedSport === "fat_bike";
  const icon =
    normalizedSport && normalizedSport !== "fat_bike"
      ? sportIcons[normalizedSport]
      : faCircleQuestion;

  return (
    <span
      role="img"
      aria-label={label}
      title={label}
      className="inline-flex h-5 w-6 shrink-0 items-center justify-center"
    >
      {customBike ? (
        <OffRoadBikeIcon fatTires={normalizedSport === "fat_bike"} />
      ) : (
        <FontAwesomeIcon
          icon={icon ?? faCircleQuestion}
          style={{ height: "100%", width: "100%" }}
          aria-hidden="true"
        />
      )}
    </span>
  );
}

function readableSportLabel(sport: string) {
  const label = sport.trim().replace(/[_-]+/g, " ");
  return label ? label.charAt(0).toUpperCase() + label.slice(1) : "Activity";
}
