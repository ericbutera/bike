"use client";

import { useState } from "react";
import toast from "react-hot-toast";
import { useAdminFeatureFlags, useUpdateFeatureFlag } from "@/lib/queries";

export default function FeatureFlagsPanel() {
  const { data: flags = [], isLoading, isError } = useAdminFeatureFlags();
  const updateFlag = useUpdateFeatureFlag();
  const [savingKeys, setSavingKeys] = useState<Record<string, boolean>>({});

  const toggleFlag = async (key: string) => {
    const currentFlag = flags.find((flag) => flag.feature_key === key);
    if (!currentFlag || savingKeys[key]) return;

    setSavingKeys((current) => ({ ...current, [key]: true }));

    try {
      await updateFlag.updateAsync(key, !currentFlag.enabled);
    } catch (error) {
      console.error("Failed to update feature flag", error);
      toast.error("Failed to update feature flag");
    } finally {
      setSavingKeys((current) => {
        const next = { ...current };
        delete next[key];
        return next;
      });
    }
  };

  if (isLoading) return <div className="p-6">Loading...</div>;

  if (isError) {
    return (
      <div className="grid gap-6">
        <FeatureFlagsHeading />
        <div className="rounded-xl border border-error/30 bg-error/5 p-6 text-sm text-error">
          Unable to load feature flags.
        </div>
      </div>
    );
  }

  return (
    <div className="grid gap-6">
      <FeatureFlagsHeading />

      <div className="overflow-hidden rounded-lg bg-base-100 shadow">
        <div className="divide-y divide-base-300">
          {flags.map((flag) => {
            const isSaving = !!savingKeys[flag.feature_key];

            return (
              <div
                key={flag.feature_key}
                className={`flex items-center justify-between gap-4 p-4 ${
                  isSaving ? "bg-warning/10" : ""
                }`}
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <h2 className="text-lg font-semibold break-all">
                      {flag.feature_key}
                    </h2>
                    {isSaving ? (
                      <span className="badge badge-warning badge-sm">
                        saving
                      </span>
                    ) : null}
                  </div>
                </div>

                <div className="flex shrink-0 items-center gap-4">
                  <span
                    className={`text-sm font-medium ${
                      flag.enabled ? "text-success" : "text-base-content/50"
                    }`}
                  >
                    {flag.enabled ? "Enabled" : "Disabled"}
                  </span>
                  <input
                    type="checkbox"
                    className="toggle toggle-success"
                    checked={flag.enabled}
                    disabled={isSaving}
                    aria-label={`Toggle ${flag.feature_key}`}
                    onChange={() => void toggleFlag(flag.feature_key)}
                  />
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

function FeatureFlagsHeading() {
  return (
    <div>
      <h1 className="text-2xl font-bold">Feature Flags</h1>
      <p className="mt-1 text-sm text-base-content/60">
        Configure which features are enabled in the application
      </p>
    </div>
  );
}
