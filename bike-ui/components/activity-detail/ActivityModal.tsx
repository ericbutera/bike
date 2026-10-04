"use client";

import { useEffect, useState } from "react";
import {
  ACTIVITY_TYPE_OPTIONS,
  normalizeActivityType,
  type ActivityType,
} from "../../lib/activityTypes";
import { useUpdateActivity } from "../../lib/queries";

export default function ActivityModal({
  activityId,
  initialTitle,
  initialActivityType,
  onClose,
}: {
  activityId: number | string;
  initialTitle: string;
  initialActivityType: string | null | undefined;
  onClose: () => void;
}) {
  const [titleDraft, setTitleDraft] = useState(initialTitle);
  const [activityTypeDraft, setActivityTypeDraft] = useState<ActivityType>(
    normalizeActivityType(initialActivityType),
  );
  const updateActivityMutation = useUpdateActivity();
  const isSaving = updateActivityMutation.isPending;
  const canSave = titleDraft.trim().length > 0 && !isSaving;

  useEffect(() => {
    setTitleDraft(initialTitle);
  }, [initialTitle]);

  useEffect(() => {
    setActivityTypeDraft(normalizeActivityType(initialActivityType));
  }, [initialActivityType]);

  async function handleSaveActivity() {
    try {
      await updateActivityMutation.updateAsync(activityId, {
        title: titleDraft,
        activity_type: activityTypeDraft,
      });
      onClose();
    } catch {
      // The mutation exposes the API error state used by the form controls.
    }
  }

  return (
    <div className="modal modal-open">
      <div className="modal-box max-w-lg">
        <h2 className="text-xl font-semibold text-base-content">
          Edit activity
        </h2>
        <form
          className="mt-5 space-y-6"
          onSubmit={(event) => {
            event.preventDefault();
            if (canSave) {
              void handleSaveActivity();
            }
          }}
        >
          <label className="form-control w-full">
            <span className="label">
              <span className="label-text font-medium">Title</span>
            </span>
            <input
              type="text"
              className="input input-bordered w-full"
              value={titleDraft}
              disabled={isSaving}
              onChange={(event) => setTitleDraft(event.target.value)}
              autoFocus
            />
          </label>

          <fieldset className="space-y-3">
            <legend className="font-medium text-base-content">
              Activity type
            </legend>
            {ACTIVITY_TYPE_OPTIONS.map((option) => (
              <label
                key={option.value}
                className={`flex cursor-pointer items-start gap-3 rounded-box border p-4 ${
                  activityTypeDraft === option.value
                    ? "border-primary bg-primary/10"
                    : "border-base-300 bg-base-100"
                }`}
              >
                <input
                  type="radio"
                  name="activity-type"
                  className="radio radio-primary mt-1"
                  value={option.value}
                  checked={activityTypeDraft === option.value}
                  disabled={isSaving}
                  onChange={(event) =>
                    setActivityTypeDraft(
                      normalizeActivityType(event.target.value),
                    )
                  }
                />
                <span>
                  <span className="block font-medium text-base-content">
                    {option.label}
                  </span>
                  <span className="mt-1 block text-sm leading-6 text-base-content/65">
                    {option.description}
                  </span>
                </span>
              </label>
            ))}
          </fieldset>

          <div className="modal-action">
            <button
              type="button"
              className="btn btn-ghost"
              disabled={isSaving}
              onClick={onClose}
            >
              Cancel
            </button>
            <button
              type="submit"
              className="btn btn-primary"
              disabled={!canSave}
            >
              {isSaving ? "Saving..." : "Save"}
            </button>
          </div>
        </form>
      </div>
      <button
        type="button"
        className="modal-backdrop"
        aria-label="Close activity modal"
        disabled={isSaving}
        onClick={onClose}
      />
    </div>
  );
}
