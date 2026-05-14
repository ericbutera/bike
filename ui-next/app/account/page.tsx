"use client";

import { auth } from "@ericbutera/kaleido";
import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";
import toast from "react-hot-toast";
import AuthRequiredCard from "../../components/AuthRequiredCard";
import IntegrationEventFeed from "../../components/IntegrationEventFeed";
import Layout from "../../components/Layout";
import {
  DEFAULT_UNIT_SYSTEM,
  extractApiMessage,
  formatActivityTimestamp,
  formatDistance,
  formatElevation,
  formatHeartRate,
  formatPower,
  formatSpeed,
  normalizeUnitSystem,
  type UnitSystem,
} from "../../lib/activityFormatting";
import {
  useDisconnectStrava,
  useQueueStravaSync,
  useStartStravaConnect,
  useStravaConnection,
  useStravaIntegrationEvents,
  useUpdateUserPreferences,
  useUserPreferences,
} from "../../lib/queries";

const UNIT_SYSTEM_OPTIONS: Array<{
  value: UnitSystem;
  label: string;
  description: string;
}> = [
  {
    value: "metric",
    label: "Metric",
    description: "Distance in km, speed in km/h, elevation in meters.",
  },
  {
    value: "imperial",
    label: "Imperial",
    description: "Distance in miles, speed in mph, elevation in feet.",
  },
  {
    value: "mixed",
    label: "Mixed",
    description: "Distance in km, speed in mph, elevation in meters.",
  },
];

const HEART_RATE_ZONE_FIELD_COUNT = 4;
const EMPTY_HEART_RATE_ZONE_DRAFT = Array.from(
  { length: HEART_RATE_ZONE_FIELD_COUNT },
  () => "",
);

const HEART_RATE_ZONE_FIELDS = [
  {
    label: "Z1 ceiling",
    description: "Anything at or below this bpm stays in Z1.",
  },
  {
    label: "Z2 ceiling",
    description: "Bike treats values above Z1 up to this bpm as Z2.",
  },
  {
    label: "Z3 ceiling",
    description: "Tempo and sweet spot efforts usually start here.",
  },
  {
    label: "Z4 ceiling",
    description: "Z5 becomes anything above this threshold.",
  },
] as const;

function zoneBoundsToDraft(bounds?: number[] | null) {
  if (!bounds || bounds.length !== HEART_RATE_ZONE_FIELD_COUNT) {
    return [...EMPTY_HEART_RATE_ZONE_DRAFT];
  }

  return bounds.map((value) => value.toString());
}

function parseOptionalIntegerInput(value: string) {
  const trimmed = value.trim();

  if (!trimmed) {
    return null;
  }

  const parsed = Number.parseInt(trimmed, 10);

  return Number.isFinite(parsed) ? parsed : null;
}

function parseHeartRateZoneBounds(draft: string[]) {
  const parsed = draft
    .map(parseOptionalIntegerInput)
    .filter((value): value is number => value != null);

  if (parsed.length === 0) {
    return null;
  }

  if (parsed.length !== HEART_RATE_ZONE_FIELD_COUNT) {
    throw new Error(
      "Enter all four heart rate zone ceilings or leave them blank.",
    );
  }

  for (let index = 1; index < parsed.length; index += 1) {
    if (parsed[index] <= parsed[index - 1]) {
      throw new Error(
        "Heart rate zone ceilings must increase from Z1 through Z4.",
      );
    }
  }

  return parsed;
}

function buildZonePreviewLabels(draft: string[]) {
  const bounds = draft.map(parseOptionalIntegerInput);

  return [
    bounds[0] != null
      ? `Z1 <= ${formatHeartRate(bounds[0])}`
      : "Z1 ceiling unset",
    bounds[0] != null && bounds[1] != null
      ? `Z2 ${formatHeartRate(bounds[0] + 1)} to ${formatHeartRate(bounds[1])}`
      : "Z2 range unset",
    bounds[1] != null && bounds[2] != null
      ? `Z3 ${formatHeartRate(bounds[1] + 1)} to ${formatHeartRate(bounds[2])}`
      : "Z3 range unset",
    bounds[2] != null && bounds[3] != null
      ? `Z4 ${formatHeartRate(bounds[2] + 1)} to ${formatHeartRate(bounds[3])}`
      : "Z4 range unset",
    bounds[3] != null ? `Z5 > ${formatHeartRate(bounds[3])}` : "Z5 range unset",
  ];
}

function formatStravaSyncStatus(status: string) {
  switch (status) {
    case "queued":
      return "Queued";
    case "running":
      return "Syncing";
    case "succeeded":
      return "Up to date";
    case "failed":
      return "Needs attention";
    default:
      return "Not synced yet";
  }
}

function isStravaSyncActive(status: string) {
  return status === "queued" || status === "running";
}

export default function AccountPage() {
  const authApi = auth.useAuthApi();
  const { user, isLoading: isLoadingUser } = authApi.useCurrentUser();
  const router = useRouter();
  const searchParams = useSearchParams();
  const preferencesQuery = useUserPreferences({ enabled: !!user });
  const stravaQuery = useStravaConnection({
    enabled: !!user,
    refetchIntervalMs: user ? 5000 : false,
  });
  const updatePreferencesMutation = useUpdateUserPreferences();
  const startStravaConnectMutation = useStartStravaConnect();
  const queueStravaSyncMutation = useQueueStravaSync();
  const disconnectStravaMutation = useDisconnectStrava();
  const unitSystem = normalizeUnitSystem(preferencesQuery.data?.unit_system);
  const estimatedFtpWatts = preferencesQuery.data?.estimated_ftp_watts ?? null;
  const heartRateZoneBounds =
    preferencesQuery.data?.heart_rate_zone_bounds_bpm ?? null;
  const stravaConnection = stravaQuery.data;
  const stravaEventsQuery = useStravaIntegrationEvents({
    enabled: !!user,
    refetchIntervalMs:
      user && isStravaSyncActive(stravaConnection.last_sync_status)
        ? 5000
        : false,
  });
  const [draftUnitSystem, setDraftUnitSystem] =
    useState<UnitSystem>(DEFAULT_UNIT_SYSTEM);
  const [draftEstimatedFtpWatts, setDraftEstimatedFtpWatts] = useState("");
  const [draftHeartRateZoneBounds, setDraftHeartRateZoneBounds] = useState(
    zoneBoundsToDraft(null),
  );

  const storedHeartRateZoneDraft = zoneBoundsToDraft(heartRateZoneBounds);
  const storedHeartRateZoneSignature = storedHeartRateZoneDraft.join("|");

  useEffect(() => {
    setDraftUnitSystem(unitSystem);
    setDraftEstimatedFtpWatts(estimatedFtpWatts?.toString() ?? "");
    setDraftHeartRateZoneBounds(storedHeartRateZoneDraft);
  }, [estimatedFtpWatts, storedHeartRateZoneSignature, unitSystem]);

  useEffect(() => {
    const status = searchParams.get("strava");

    if (!status) {
      return;
    }

    const message =
      searchParams.get("strava_message") ??
      (status === "connected"
        ? "Strava connected. Initial sync queued."
        : "Strava connection failed.");

    if (status === "connected") {
      toast.success(message);
    } else {
      toast.error(message);
    }

    router.replace("/account");
  }, [router, searchParams]);

  async function handleSave() {
    try {
      const nextHeartRateZoneBounds = parseHeartRateZoneBounds(
        draftHeartRateZoneBounds,
      );

      await updatePreferencesMutation.updateAsync({
        unit_system: draftUnitSystem,
        estimated_ftp_watts: parseOptionalIntegerInput(draftEstimatedFtpWatts),
        heart_rate_zone_bounds_bpm: nextHeartRateZoneBounds,
      });
      toast.success("Preferences saved.");
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : extractApiMessage(error),
      );
    }
  }

  async function handleStartStravaConnect() {
    try {
      const result = await startStravaConnectMutation.beginAsync();
      window.location.assign(result.authorization_url);
    } catch (error) {
      toast.error(extractApiMessage(error));
    }
  }

  async function handleQueueStravaSync() {
    try {
      await queueStravaSyncMutation.queueAsync();
      toast.success("Strava sync queued.");
    } catch (error) {
      toast.error(extractApiMessage(error));
    }
  }

  async function handleDisconnectStrava() {
    try {
      const result = await disconnectStravaMutation.disconnectAsync();
      toast.success(result.message);
    } catch (error) {
      toast.error(extractApiMessage(error));
    }
  }

  const isDirty =
    draftUnitSystem !== unitSystem ||
    draftEstimatedFtpWatts !== (estimatedFtpWatts?.toString() ?? "") ||
    draftHeartRateZoneBounds.join("|") !== storedHeartRateZoneDraft.join("|");
  const zonePreviewLabels = buildZonePreviewLabels(draftHeartRateZoneBounds);
  const isStravaSyncPending = isStravaSyncActive(
    stravaConnection.last_sync_status,
  );

  return (
    <Layout>
      <section className="mx-auto max-w-5xl px-4 py-8 sm:px-6 lg:px-8">
        {isLoadingUser ||
        (user && (preferencesQuery.isLoading || stravaQuery.isLoading)) ? (
          <div className="card bg-base-100 shadow-xl">
            <div className="card-body items-center py-10">
              <span className="loading loading-spinner loading-md" />
            </div>
          </div>
        ) : !user ? (
          <AuthRequiredCard
            eyebrow="Account"
            title="Sign in to manage preferences"
            description="Bike stores unit preferences, heart rate zones, and FTP estimates per account so activity analysis stays consistent."
          />
        ) : (
          <div className="space-y-6">
            <div>
              <p className="text-sm uppercase tracking-[0.22em] text-base-content/50">
                Account
              </p>
              <h1 className="mt-2 text-4xl font-semibold">Preferences</h1>
              <p className="mt-3 max-w-2xl text-sm leading-6 text-base-content/70">
                Choose how Bike formats units and define the training profile
                Bike uses for ride-level zone summaries and future load models.
              </p>
            </div>

            <div className="card bg-base-100 shadow-xl">
              <div className="card-body gap-6">
                <div className="flex flex-wrap items-start justify-between gap-4">
                  <div>
                    <h2 className="card-title text-xl">Strava integration</h2>
                    <p className="text-sm text-base-content/70">
                      Connect a Strava athlete so Bike can pull new activities
                      in the background and feed them through the same import
                      pipeline as manual uploads.
                    </p>
                  </div>
                  <span
                    className={`badge ${stravaConnection.connected ? "badge-primary" : "badge-outline"}`}
                  >
                    {stravaConnection.connected ? "Connected" : "Not connected"}
                  </span>
                </div>

                {!stravaConnection.configured ? (
                  <div className="rounded-box border border-warning/30 bg-warning/10 p-4 text-sm text-base-content/80">
                    This Bike deployment does not have Strava OAuth credentials
                    configured yet. Set <code>STRAVA_CLIENT_ID</code> and
                    <code>STRAVA_CLIENT_SECRET</code> on the API and worker.
                  </div>
                ) : null}

                <div className="grid gap-4 lg:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
                  <div className="rounded-box bg-base-200 p-4 text-sm text-base-content/75">
                    {stravaConnection.connected ? (
                      <div className="space-y-4">
                        <div>
                          <div className="font-medium text-base-content">
                            {stravaConnection.athlete_name ??
                              "Connected athlete"}
                          </div>
                          <div className="mt-1 text-sm text-base-content/70">
                            {stravaConnection.athlete_username
                              ? `@${stravaConnection.athlete_username}`
                              : "Strava account linked to this Bike user."}
                          </div>
                        </div>

                        {stravaConnection.scopes.length > 0 ? (
                          <div className="flex flex-wrap gap-2">
                            {stravaConnection.scopes.map((scope) => (
                              <span
                                key={scope}
                                className="badge badge-ghost badge-sm"
                              >
                                {scope}
                              </span>
                            ))}
                          </div>
                        ) : null}

                        <div className="rounded-box border border-base-300 bg-base-100 p-4">
                          <div className="font-medium text-base-content">
                            {formatStravaSyncStatus(
                              stravaConnection.last_sync_status,
                            )}
                          </div>
                          <div className="mt-2 space-y-2 text-sm text-base-content/70">
                            <div>
                              {stravaConnection.last_sync_message ??
                                "Bike has the connection and is ready to sync."}
                            </div>
                            <div>
                              Last finished:{" "}
                              {formatActivityTimestamp(
                                stravaConnection.last_sync_finished_at ?? "",
                              )}
                            </div>
                            <div>
                              Cursor:{" "}
                              {formatActivityTimestamp(
                                stravaConnection.last_synced_activity_started_at ??
                                  "",
                              )}
                            </div>
                          </div>
                        </div>
                      </div>
                    ) : (
                      <div className="space-y-3 leading-6">
                        <p>
                          Bike requests Strava activity access and stores the
                          refresh token so new syncs can happen without asking
                          you to reconnect each time.
                        </p>
                        <p>
                          The first sync backfills your available activities and
                          runs them through Bike&apos;s existing dedupe and
                          import pipeline.
                        </p>
                      </div>
                    )}
                  </div>

                  <div className="grid gap-3">
                    <div className="stats stats-vertical bg-base-200 shadow-sm lg:stats-horizontal">
                      <div className="stat px-4 py-3">
                        <div className="stat-title">Imported</div>
                        <div className="stat-value text-2xl">
                          {stravaConnection.last_sync_imported_count}
                        </div>
                      </div>
                      <div className="stat px-4 py-3">
                        <div className="stat-title">Duplicates</div>
                        <div className="stat-value text-2xl">
                          {stravaConnection.last_sync_duplicate_count}
                        </div>
                      </div>
                      <div className="stat px-4 py-3">
                        <div className="stat-title">Failed</div>
                        <div className="stat-value text-2xl">
                          {stravaConnection.last_sync_failed_count}
                        </div>
                      </div>
                    </div>

                    <div className="rounded-box border border-base-300 bg-base-200 p-4 text-sm text-base-content/70">
                      <div className="font-medium text-base-content">
                        Sync behavior
                      </div>
                      <p className="mt-2 leading-6">
                        Bike pulls activity summaries and streams from Strava,
                        synthesizes a TCX payload, then reuses the existing
                        upload pipeline so dedupe and derived metrics stay in
                        one place.
                      </p>
                    </div>
                  </div>
                </div>

                <div className="card-actions justify-end gap-3">
                  {stravaConnection.connected ? (
                    <>
                      <button
                        type="button"
                        className="btn btn-ghost"
                        disabled={
                          disconnectStravaMutation.isPending ||
                          startStravaConnectMutation.isPending
                        }
                        onClick={handleDisconnectStrava}
                      >
                        {disconnectStravaMutation.isPending
                          ? "Disconnecting..."
                          : "Disconnect"}
                      </button>
                      <button
                        type="button"
                        className="btn btn-outline"
                        disabled={
                          isStravaSyncPending ||
                          queueStravaSyncMutation.isPending ||
                          disconnectStravaMutation.isPending
                        }
                        onClick={handleQueueStravaSync}
                      >
                        {isStravaSyncPending ||
                        queueStravaSyncMutation.isPending
                          ? "Sync queued..."
                          : "Sync now"}
                      </button>
                    </>
                  ) : (
                    <button
                      type="button"
                      className="btn btn-primary"
                      disabled={
                        !stravaConnection.configured ||
                        startStravaConnectMutation.isPending ||
                        disconnectStravaMutation.isPending
                      }
                      onClick={handleStartStravaConnect}
                    >
                      {startStravaConnectMutation.isPending
                        ? "Redirecting..."
                        : "Connect Strava"}
                    </button>
                  )}
                </div>
              </div>
            </div>

            <div className="card bg-base-100 shadow-xl">
              <div className="card-body gap-6">
                <div>
                  <h2 className="card-title text-xl">Strava sync history</h2>
                  <p className="text-sm text-base-content/70">
                    Recent OAuth, webhook, sync, and disconnect events for this
                    account.
                  </p>
                </div>

                <IntegrationEventFeed
                  events={stravaEventsQuery.data}
                  isLoading={stravaEventsQuery.isLoading}
                  error={stravaEventsQuery.error}
                  emptyMessage="No Strava history yet. Connect Strava or queue a sync to start recording events."
                />
              </div>
            </div>

            <div className="card bg-base-100 shadow-xl">
              <div className="card-body gap-6">
                <div>
                  <h2 className="card-title text-xl">Units</h2>
                  <p className="text-sm text-base-content/70">
                    This preference is stored in Bike and applied anywhere those
                    values are rendered.
                  </p>
                </div>

                <div className="grid gap-3 lg:grid-cols-3">
                  {UNIT_SYSTEM_OPTIONS.map((option) => {
                    const isSelected = draftUnitSystem === option.value;

                    return (
                      <label
                        key={option.value}
                        className={`card cursor-pointer border transition ${isSelected ? "border-primary bg-primary/5 shadow-md" : "border-base-300 bg-base-200/70 shadow-sm"}`}
                      >
                        <div className="card-body gap-3 p-4">
                          <div className="flex items-start justify-between gap-3">
                            <div>
                              <div className="font-semibold text-base-content">
                                {option.label}
                              </div>
                              <p className="mt-1 text-sm text-base-content/70">
                                {option.description}
                              </p>
                            </div>
                            <input
                              type="radio"
                              name="unit-system"
                              className="radio radio-primary radio-sm mt-1"
                              checked={isSelected}
                              onChange={() => {
                                setDraftUnitSystem(option.value);
                              }}
                            />
                          </div>
                        </div>
                      </label>
                    );
                  })}
                </div>

                <div className="rounded-box bg-base-200 p-4 text-sm text-base-content/75">
                  <div className="font-medium text-base-content">Preview</div>
                  <div className="mt-2 flex flex-wrap gap-4">
                    <span>{`Distance ${formatDistance(40233, draftUnitSystem)}`}</span>
                    <span>{`Speed ${formatSpeed(8.94, draftUnitSystem)}`}</span>
                    <span>{`Elevation ${formatElevation(512, draftUnitSystem)}`}</span>
                  </div>
                </div>
              </div>
            </div>

            <div className="card bg-base-100 shadow-xl">
              <div className="card-body gap-6">
                <div>
                  <h2 className="card-title text-xl">Training profile</h2>
                  <p className="text-sm text-base-content/70">
                    Heart rate zones are stored in Bike and written onto new or
                    regenerated rides. Estimated FTP is kept alongside them so
                    future power-based analysis has a stable snapshot.
                  </p>
                </div>

                <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.3fr)]">
                  <label className="form-control">
                    <div className="label">
                      <span className="label-text font-medium">
                        Estimated FTP
                      </span>
                      <span className="label-text-alt">Optional</span>
                    </div>
                    <input
                      type="number"
                      min={80}
                      max={600}
                      className="input input-bordered"
                      placeholder="265"
                      value={draftEstimatedFtpWatts}
                      onChange={(event) => {
                        setDraftEstimatedFtpWatts(event.target.value);
                      }}
                    />
                    <div className="label">
                      <span className="label-text-alt text-base-content/60">
                        Stored now for power zones and future load estimates
                        once Bike carries power data.
                      </span>
                    </div>
                  </label>

                  <div className="rounded-box bg-base-200 p-4 text-sm text-base-content/75">
                    <div className="font-medium text-base-content">
                      Training preview
                    </div>
                    <div className="mt-2 flex flex-wrap gap-4">
                      <span>{`FTP ${formatPower(parseOptionalIntegerInput(draftEstimatedFtpWatts))}`}</span>
                    </div>
                    <div className="mt-4 grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
                      {zonePreviewLabels.map((label) => (
                        <div
                          key={label}
                          className="rounded-lg border border-base-300 bg-base-100 px-3 py-2"
                        >
                          {label}
                        </div>
                      ))}
                    </div>
                  </div>
                </div>

                <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
                  {HEART_RATE_ZONE_FIELDS.map((field, index) => (
                    <label key={field.label} className="form-control">
                      <div className="label">
                        <span className="label-text font-medium">
                          {field.label}
                        </span>
                      </div>
                      <input
                        type="number"
                        min={40}
                        max={240}
                        className="input input-bordered"
                        placeholder={`${120 + index * 15}`}
                        value={draftHeartRateZoneBounds[index]}
                        onChange={(event) => {
                          setDraftHeartRateZoneBounds((current) => {
                            const next = [...current];
                            next[index] = event.target.value;
                            return next;
                          });
                        }}
                      />
                      <div className="label">
                        <span className="label-text-alt text-base-content/60">
                          {field.description}
                        </span>
                      </div>
                    </label>
                  ))}
                </div>

                <div className="rounded-box bg-base-200 p-4 text-sm text-base-content/75">
                  <div className="font-medium text-base-content">
                    How Bike uses this
                  </div>
                  <p className="mt-2 leading-6">
                    New uploads and regenerated rides will persist per-ride
                    heart rate zone time. Existing rides keep their historical
                    snapshot until you regenerate them.
                  </p>
                </div>

                <div className="card-actions justify-end">
                  <button
                    type="button"
                    className="btn btn-ghost"
                    disabled={!isDirty || updatePreferencesMutation.isPending}
                    onClick={() => {
                      setDraftUnitSystem(unitSystem);
                      setDraftEstimatedFtpWatts(
                        estimatedFtpWatts?.toString() ?? "",
                      );
                      setDraftHeartRateZoneBounds(storedHeartRateZoneDraft);
                    }}
                  >
                    Reset
                  </button>
                  <button
                    type="button"
                    className="btn btn-primary"
                    disabled={!isDirty || updatePreferencesMutation.isPending}
                    onClick={handleSave}
                  >
                    {updatePreferencesMutation.isPending
                      ? "Saving..."
                      : "Save preferences"}
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}
      </section>
    </Layout>
  );
}
