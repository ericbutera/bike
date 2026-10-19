import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import ActivitySportIcon from "../ActivitySportIcon";

describe("ActivitySportIcon", () => {
  it.each([
    ["walk", "Walk", "person-walking"],
    ["Walking", "Walk", "person-walking"],
    ["hike", "Hike", "person-hiking"],
    ["hiking", "Hike", "person-hiking"],
    ["run", "Run", "person-running"],
    ["TrailRun", "Run", "person-running"],
    ["road_ride", "Road ride", "bicycle"],
    ["ride", "Road ride", "bicycle"],
    ["bike", "Road ride", "bicycle"],
    ["road cycling", "Road ride", "bicycle"],
    ["indoor_trainer_ride", "Indoor trainer ride", "person-biking"],
    ["VirtualRide", "Indoor trainer ride", "person-biking"],
    ["swim", "Swim", "person-swimming"],
    ["workout", "Workout", "dumbbell"],
    ["stand_up_paddle", "Stand up paddle", "circle-question"],
    ["", "Activity", "circle-question"],
  ])("shows %s with a labeled %s icon", (sport, label, iconName) => {
    render(<ActivitySportIcon sport={sport} />);

    const icon = screen.getByRole("img", { name: label });
    expect(icon).toHaveAttribute("title", label);
    expect(icon.querySelector("svg")).toHaveAttribute("data-icon", iconName);
    expect(icon.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  it.each(["mountain_bike", "mountain_biking", "MountainBikeRide", "mtb"])(
    "shows original mountain-bike artwork for %s",
    (sport) => {
      render(<ActivitySportIcon sport={sport} />);

      const icon = screen.getByRole("img", { name: "Mountain bike" });
      expect(icon).toHaveAttribute("title", "Mountain bike");
      expect(icon.querySelector("svg")).not.toHaveAttribute("data-prefix");
      expect(icon.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
      expect(icon.querySelector("g")).toHaveAttribute("stroke-width", "3.5");
    },
  );

  it.each(["fat_bike", "fat tire biking", "FatBikeRide"])(
    "shows original fat-bike artwork with wider tires for %s",
    (sport) => {
      render(<ActivitySportIcon sport={sport} />);

      const icon = screen.getByRole("img", { name: "Fat tire biking" });
      expect(icon).toHaveAttribute("title", "Fat tire biking");
      expect(icon.querySelector("svg")).not.toHaveAttribute("data-prefix");
      expect(icon.querySelector("g")).toHaveAttribute("stroke-width", "6");
    },
  );
});
