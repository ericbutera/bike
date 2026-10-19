"use client";

import { useId, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { buildActivityClimbs } from "../../lib/activityClimbs";
import type { UnitSystem } from "../../lib/activityFormatting";
import type { Activity } from "./types";
import MatchedSegmentsSection from "../MatchedSegmentsSection";
import { AppCard } from "../ui/Card";
import ActivityClimbsPanel from "./ActivityClimbsPanel";
import { useMatchedSegmentGroups } from "./matchedSegments";

type DetailTab = { key: "segments" | "climbs"; label: string; count: number };

export default function ActivityRouteDetails({
  activity,
  unitSystem,
  selectedSegmentIds,
  onToggleSegmentMatch,
}: {
  activity: Activity;
  unitSystem: UnitSystem;
  selectedSegmentIds: number[];
  onToggleSegmentMatch: (segmentId: number) => void;
}) {
  const id = useId();
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const segments = useMatchedSegmentGroups(activity);
  const climbs = useMemo(
    () => buildActivityClimbs(activity.route_points),
    [activity.route_points],
  );
  const choices: DetailTab[] = [
    { key: "segments", label: "Segments", count: segments.length },
    { key: "climbs", label: "Climbs", count: climbs.length },
  ];
  const tabs = choices.filter((tab) => tab.count > 0);
  const [activeTab, setActiveTab] = useState<DetailTab["key"]>(
    tabs[0]?.key ?? "segments",
  );
  const selectedTab =
    tabs.find((tab) => tab.key === activeTab)?.key ?? tabs[0]?.key;

  function handleTabKeyDown(
    event: KeyboardEvent<HTMLButtonElement>,
    index: number,
  ) {
    let nextIndex: number;
    switch (event.key) {
      case "ArrowRight":
        nextIndex = (index + 1) % tabs.length;
        break;
      case "ArrowLeft":
        nextIndex = (index + tabs.length - 1) % tabs.length;
        break;
      case "Home":
        nextIndex = 0;
        break;
      case "End":
        nextIndex = tabs.length - 1;
        break;
      default:
        return;
    }
    event.preventDefault();
    setActiveTab(tabs[nextIndex].key);
    tabRefs.current[nextIndex]?.focus();
  }

  if (tabs.length === 0) return null;

  return (
    <AppCard
      as="section"
      aria-label="Segments and climbs"
      bodyClassName="gap-4"
    >
      <div
        role="tablist"
        aria-label="Activity route details"
        className="tabs tabs-border"
      >
        {tabs.map((tab, index) => (
          <button
            key={tab.key}
            ref={(node) => {
              tabRefs.current[index] = node;
            }}
            id={`${id}-${tab.key}-tab`}
            type="button"
            role="tab"
            className={`tab gap-2 ${selectedTab === tab.key ? "tab-active" : ""}`}
            aria-label={`${tab.label} (${tab.count})`}
            aria-selected={selectedTab === tab.key}
            aria-controls={`${id}-${tab.key}-panel`}
            tabIndex={selectedTab === tab.key ? 0 : -1}
            onClick={() => setActiveTab(tab.key)}
            onKeyDown={(event) => handleTabKeyDown(event, index)}
          >
            {tab.label}
            <span className="text-xs tabular-nums opacity-60">{tab.count}</span>
          </button>
        ))}
      </div>
      {tabs.map((tab) => (
        <div
          key={tab.key}
          id={`${id}-${tab.key}-panel`}
          role="tabpanel"
          aria-labelledby={`${id}-${tab.key}-tab`}
          tabIndex={0}
          hidden={selectedTab !== tab.key}
        >
          {tab.key === "segments" ? (
            <MatchedSegmentsSection
              activityId={activity.id}
              selectedSegmentIds={selectedSegmentIds}
              onToggleSegmentMatch={onToggleSegmentMatch}
            />
          ) : (
            <ActivityClimbsPanel
              activityId={activity.id}
              climbs={climbs}
              unitSystem={unitSystem}
              active={selectedTab === "climbs"}
            />
          )}
        </div>
      ))}
    </AppCard>
  );
}
