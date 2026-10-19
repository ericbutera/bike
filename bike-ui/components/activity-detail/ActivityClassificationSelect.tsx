import { faChevronDown } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import {
  ACTIVITY_SPORT_OPTIONS,
  type ActivitySport,
} from "../../lib/activitySports";
import {
  ACTIVITY_TYPE_OPTIONS,
  normalizeActivityType,
  type ActivityType,
} from "../../lib/activityTypes";
import { formatSport } from "../../lib/activityFormatting";
import ActivitySportIcon, { resolveActivitySport } from "../ActivitySportIcon";
import ActionMenu from "../ui/ActionMenu";

export function ActivitySportSelect({
  value,
  onChange,
  disabled = false,
  iconOnly = false,
}: {
  value: string;
  onChange: (sport: ActivitySport) => void;
  disabled?: boolean;
  iconOnly?: boolean;
}) {
  const selected = resolveActivitySport(value);
  const label =
    ACTIVITY_SPORT_OPTIONS.find((option) => option.value === selected)?.label ??
    formatSport(value);
  return (
    <ActionMenu
      label={`${iconOnly ? "Change activity type" : "Activity type"}: ${label}`}
      disabled={disabled}
      triggerClassName={
        iconOnly
          ? "btn btn-ghost btn-square btn-sm"
          : "btn btn-sm justify-between border-base-300 bg-base-100 font-normal"
      }
      triggerContent={
        <>
          <span aria-hidden="true">
            <ActivitySportIcon sport={value} />
          </span>
          {iconOnly ? null : (
            <>
              {label}
              <FontAwesomeIcon icon={faChevronDown} aria-hidden="true" />
            </>
          )}
        </>
      }
      items={ACTIVITY_SPORT_OPTIONS.map((option) => ({
        label: option.label,
        icon: <ActivitySportIcon sport={option.value} />,
        selected: selected === option.value,
        onSelect: () => onChange(option.value),
      }))}
    />
  );
}

export function ActivityTypeSelect({
  value,
  onChange,
  disabled = false,
}: {
  value: string | null | undefined;
  onChange: (type: ActivityType) => void;
  disabled?: boolean;
}) {
  const selected = normalizeActivityType(value);
  const label = ACTIVITY_TYPE_OPTIONS.find(
    (option) => option.value === selected,
  )!.label;
  return (
    <ActionMenu
      label={`Change training/race: ${label}`}
      disabled={disabled}
      triggerClassName="badge badge-outline h-8 gap-2 border-base-300 px-3"
      triggerContent={
        <>
          {label}
          <FontAwesomeIcon
            icon={faChevronDown}
            className="h-2.5 w-2.5"
            aria-hidden="true"
          />
        </>
      }
      items={ACTIVITY_TYPE_OPTIONS.map((option) => ({
        label: option.label,
        selected: selected === option.value,
        onSelect: () => onChange(option.value),
      }))}
    />
  );
}
