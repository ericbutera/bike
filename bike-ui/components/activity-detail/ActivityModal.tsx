"use client";

import { useId, type ChangeEvent } from "react";
import {
  ACTIVITY_TYPE_OPTIONS,
  normalizeActivityType,
} from "../../lib/activityTypes";
import {
  ACTIVITY_SPORT_OPTIONS,
  parseActivitySport,
} from "../../lib/activitySports";
import {
  useUpdateActivity,
  type Activity,
  type UpdateActivityInput,
} from "../../lib/queries";
import { useKeyedState } from "../../lib/useKeyedState";
import { resolveActivitySport } from "../ActivitySportIcon";

export default function ActivityModal({
  activity,
  onClose,
}: {
  activity: Activity;
  onClose: () => void;
}) {
  const sportLabelId = useId();
  const knownSport = parseActivitySport(
    resolveActivitySport(activity.sport) ?? null,
  );
  const initial = {
    title: activity.title,
    sport: knownSport ?? activity.sport,
    activity_type: normalizeActivityType(activity.activity_type),
    location: activity.location ?? "",
    description: activity.description ?? "",
    private_notes: activity.private_notes ?? "",
    gear_name: activity.gear?.name ?? "",
  };
  const [draft, setDraft] = useKeyedState(
    `${activity.id}:${JSON.stringify(initial)}`,
    initial,
  );
  const update = useUpdateActivity();
  const patch: UpdateActivityInput = {};
  if (draft.title !== initial.title) patch.title = draft.title;
  if (draft.sport !== initial.sport)
    patch.sport = parseActivitySport(draft.sport);
  if (draft.activity_type !== initial.activity_type)
    patch.activity_type = draft.activity_type;
  if (draft.location !== initial.location) patch.location = draft.location;
  if (draft.description !== initial.description)
    patch.description = draft.description;
  if (draft.private_notes !== initial.private_notes)
    patch.private_notes = draft.private_notes;
  if (draft.gear_name !== initial.gear_name) patch.gear_name = draft.gear_name;
  const canSave =
    draft.title.trim().length > 0 && Object.keys(patch).length > 0;

  async function save() {
    try {
      await update.updateAsync(activity.id, patch);
      onClose();
    } catch {
      /* Keep the draft and show the mutation error below. */
    }
  }

  return (
    <div
      className="modal modal-open"
      role="dialog"
      aria-modal="true"
      aria-label="Edit activity"
    >
      <div className="modal-box max-w-lg">
        <h2 className="text-xl font-semibold text-base-content">
          Edit activity
        </h2>
        <form
          className="mt-4 space-y-4"
          onSubmit={(event) => {
            event.preventDefault();
            if (canSave && !update.isPending) void save();
          }}
        >
          <ActivityTextField
            label="Title"
            autoFocus
            value={draft.title}
            disabled={update.isPending}
            onChange={(title) => setDraft((current) => ({ ...current, title }))}
          />
          <label className="fieldset w-full">
            <span id={sportLabelId} className="label">
              Activity type
            </span>
            <select
              aria-labelledby={sportLabelId}
              className="select w-full"
              value={draft.sport}
              disabled={update.isPending}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  sport: event.target.value,
                }))
              }
            >
              {!knownSport ? (
                <option value={activity.sport}>{activity.sport}</option>
              ) : null}
              {ACTIVITY_SPORT_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          <fieldset className="fieldset">
            <legend className="label">Training / race</legend>
            <div className="flex flex-wrap gap-6">
              {ACTIVITY_TYPE_OPTIONS.map((option) => (
                <label
                  key={option.value}
                  className="flex cursor-pointer items-center gap-2"
                >
                  <input
                    type="radio"
                    name="activity-purpose"
                    className="radio radio-sm"
                    value={option.value}
                    checked={draft.activity_type === option.value}
                    disabled={update.isPending}
                    onChange={() =>
                      setDraft((current) => ({
                        ...current,
                        activity_type: option.value,
                      }))
                    }
                  />
                  {option.label}
                </label>
              ))}
            </div>
          </fieldset>
          <ActivityTextField
            label="Location"
            value={draft.location}
            disabled={update.isPending}
            onChange={(location) =>
              setDraft((current) => ({ ...current, location }))
            }
          />
          <ActivityTextField
            label="Description"
            multiline
            value={draft.description}
            disabled={update.isPending}
            onChange={(description) =>
              setDraft((current) => ({ ...current, description }))
            }
          />
          {activity.gear ? (
            <ActivityTextField
              label="Gear name"
              value={draft.gear_name}
              disabled={update.isPending}
              onChange={(gear_name) =>
                setDraft((current) => ({ ...current, gear_name }))
              }
            />
          ) : null}
          {activity.private_notes != null ? (
            <ActivityTextField
              label="Private notes"
              multiline
              value={draft.private_notes}
              disabled={update.isPending}
              onChange={(private_notes) =>
                setDraft((current) => ({ ...current, private_notes }))
              }
            />
          ) : null}
          {update.isError ? (
            <p role="alert" className="text-sm text-error">
              Unable to save activity. Your changes are still here; please try
              again.
            </p>
          ) : null}
          <div className="modal-action">
            <button
              type="button"
              className="btn btn-ghost"
              disabled={update.isPending}
              onClick={onClose}
            >
              Cancel
            </button>
            <button
              type="submit"
              className="btn btn-primary"
              disabled={!canSave || update.isPending}
            >
              {update.isPending ? "Saving..." : "Save"}
            </button>
          </div>
        </form>
      </div>
      <button
        type="button"
        className="modal-backdrop"
        aria-label="Close activity modal"
        disabled={update.isPending}
        onClick={onClose}
      />
    </div>
  );
}

function ActivityTextField({
  label,
  value,
  onChange,
  disabled,
  multiline = false,
  autoFocus = false,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  disabled: boolean;
  multiline?: boolean;
  autoFocus?: boolean;
}) {
  const id = useId();
  const control = {
    id,
    value,
    disabled,
    onChange: (event: ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) =>
      onChange(event.target.value),
  };
  return (
    <div className="fieldset w-full">
      <label htmlFor={id} className="label">
        {label}
      </label>
      {multiline ? (
        <textarea {...control} className="textarea w-full" rows={3} />
      ) : (
        <input
          {...control}
          type="text"
          autoFocus={autoFocus}
          className="input w-full"
        />
      )}
    </div>
  );
}
