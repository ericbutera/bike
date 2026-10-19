import { faChevronDown, faLayerGroup } from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import {
  ACTIVITY_SPORT_OPTIONS,
  type ActivitySport,
} from "../../lib/activitySports";
import ActivitySportIcon from "../ActivitySportIcon";
import ActionMenu from "../ui/ActionMenu";

export default function ActivitySportFilter({
  value,
  onChange,
}: {
  value?: ActivitySport;
  onChange: (value: string) => void;
}) {
  const selected = ACTIVITY_SPORT_OPTIONS.find(
    (option) => option.value === value,
  );
  const label = selected?.label ?? "All activity types";
  const allIcon = (
    <span className="inline-flex h-5 w-6 items-center justify-center">
      <FontAwesomeIcon icon={faLayerGroup} aria-hidden="true" />
    </span>
  );

  return (
    <ActionMenu
      label={`Activity type: ${label}`}
      triggerClassName="btn btn-sm gap-2 border-base-300 bg-base-100 font-normal"
      triggerContent={
        <>
          <span aria-hidden="true">
            {value ? <ActivitySportIcon sport={value} /> : allIcon}
          </span>
          {label}
          <FontAwesomeIcon icon={faChevronDown} aria-hidden="true" />
        </>
      }
      items={[
        {
          label: "All activity types",
          icon: allIcon,
          selected: !value,
          onSelect: () => onChange(""),
        },
        ...ACTIVITY_SPORT_OPTIONS.map((option) => ({
          label: option.label,
          icon: <ActivitySportIcon sport={option.value} />,
          selected: option.value === value,
          onSelect: () => onChange(option.value),
        })),
      ]}
    />
  );
}
