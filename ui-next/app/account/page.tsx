"use client";

import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";
import toast from "react-hot-toast";
import IntegrationEventFeed from "../../components/IntegrationEventFeed";
import Layout from "../../components/Layout";
import RequireAuth from "../../components/RequireAuth";
import { AppCard, CardHeader } from "../../components/ui/Card";
import InfoTooltip from "../../components/ui/InfoTooltip";
import { LoadingCard } from "../../components/ui/QueryState";
import {
  DEFAULT_UNIT_SYSTEM,
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
  useCompleteGarminIqLink,
  useDisconnectStrava,
  useGarminIqLinkedDevices,
  useQueueStravaSync,
  useStartStravaConnect,
  useStravaConnection,
  useStravaIntegrationEvents,
  useUnlinkGarminIqDevice,
  useUpdateUserPreferences,
  useUserPreferences,
} from "../../lib/queries";
import {
  calculateHeartRateZoneBoundsFromMaxHeartRate,
  hasConfiguredHeartRateZoneBounds,
  MAX_MAX_HEART_RATE_BPM,
  MIN_MAX_HEART_RATE_BPM,
} from "../../lib/trainingProfile";

const ACCOUNT_PREFERENCES_HELP_TEXT =
  "Choose how Bike formats units and define the training profile Bike uses for ride-level zone summaries and future load models.";
const STRAVA_INTEGRATION_HELP_TEXT =
  "Connect a Strava athlete so Bike can pull new activities in the background and feed them through the same import pipeline as manual uploads.";
const UNITS_HELP_TEXT =
  "This preference is stored in Bike and applied anywhere those values are rendered.";
const TRAINING_PROFILE_HELP_TEXT =
  "Heart rate zones are stored in Bike and written onto new or regenerated rides. Estimated FTP is kept alongside them so future power-based analysis has a stable snapshot.";
const TRAINING_PROFILE_USAGE_HELP_TEXT =
  "New uploads and regenerated rides will persist per-ride heart rate zone time. Existing rides keep their historical snapshot until you regenerate them.";

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
  return (
    <Layout>
      <section className="mx-auto max-w-5xl px-4 py-8 sm:px-6 lg:px-8">
        <RequireAuth>
          <AuthenticatedAccountPage />
        </RequireAuth>
      </section>
    </Layout>
  );
}

function AuthenticatedAccountPage() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const preferencesQuery = useUserPreferences();
  const stravaQuery = useStravaConnection({
    refetchIntervalMs: 5000,
  });
  const updatePreferencesMutation = useUpdateUserPreferences();
  const startStravaConnectMutation = useStartStravaConnect();
  const queueStravaSyncMutation = useQueueStravaSync();
  const disconnectStravaMutation = useDisconnectStrava();
  const garminDevicesQuery = useGarminIqLinkedDevices();
  const completeGarminLinkMutation = useCompleteGarminIqLink();
  const unlinkGarminDeviceMutation = useUnlinkGarminIqDevice();
  const unitSystem = normalizeUnitSystem(preferencesQuery.data?.unit_system);
  const estimatedFtpWatts = preferencesQuery.data?.estimated_ftp_watts ?? null;
  const heartRateZoneBounds =
    preferencesQuery.data?.heart_rate_zone_bounds_bpm ?? null;
  const stravaConnection = stravaQuery.data;
  const stravaEventsQuery = useStravaIntegrationEvents({
    refetchIntervalMs: isStravaSyncActive(stravaConnection.last_sync_status)
      ? 5000
      : false,
  });
  const [draftUnitSystem, setDraftUnitSystem] =
    useState<UnitSystem>(DEFAULT_UNIT_SYSTEM);
  const [draftEstimatedFtpWatts, setDraftEstimatedFtpWatts] = useState("");
  const [garminPairingCode, setGarminPairingCode] = useState("");
  const [draftMaxHeartRate, setDraftMaxHeartRate] = useState("");
  const [draftHeartRateZoneBounds, setDraftHeartRateZoneBounds] = useState(
    zoneBoundsToDraft(null),
  );
  const heartRateZonesConfigured =
    hasConfiguredHeartRateZoneBounds(heartRateZoneBounds);

  const storedHeartRateZoneDraft = zoneBoundsToDraft(heartRateZoneBounds);
  const storedHeartRateZoneSignature = storedHeartRateZoneDraft.join("|");

  useEffect(() => {
    setDraftUnitSystem(unitSystem);
    setDraftEstimatedFtpWatts(estimatedFtpWatts?.toString() ?? "");
    setDraftMaxHeartRate("");
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

  useEffect(() => {
    const pairingCode = searchParams.get("garmin_pair");

    if (!pairingCode) {
      return;
    }

    setGarminPairingCode(pairingCode.toUpperCase());
  }, [searchParams]);

  async function handleSave() {
    let nextHeartRateZoneBounds;

    try {
      nextHeartRateZoneBounds = parseHeartRateZoneBounds(
        draftHeartRateZoneBounds,
      );
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Invalid heart rate zones.",
      );
      return;
    }

    try {
      await updatePreferencesMutation.updateAsync({
        unit_system: draftUnitSystem,
        estimated_ftp_watts: parseOptionalIntegerInput(draftEstimatedFtpWatts),
        heart_rate_zone_bounds_bpm: nextHeartRateZoneBounds,
        xc_goal_event_name: preferencesQuery.data?.xc_goal_event_name ?? null,
        xc_goal_start_date: preferencesQuery.data?.xc_goal_start_date ?? null,
        xc_goal_target_date: preferencesQuery.data?.xc_goal_target_date ?? null,
        xc_goal_target_distance_meters:
          preferencesQuery.data?.xc_goal_target_distance_meters ?? null,
        xc_goal_target_elevation_gain_meters:
          preferencesQuery.data?.xc_goal_target_elevation_gain_meters ?? null,
        xc_goal_target_finish_time_seconds:
          preferencesQuery.data?.xc_goal_target_finish_time_seconds ?? null,
        xc_goal_event_profile:
          preferencesQuery.data?.xc_goal_event_profile ?? null,
      });
      toast.success("Preferences saved.");
    } catch {
      // Mutation errors are surfaced by the app-level React Query handler.
    }
  }

  function handleCalculateHeartRateZones() {
    try {
      const parsedMaxHeartRate = parseOptionalIntegerInput(draftMaxHeartRate);

      if (parsedMaxHeartRate == null) {
        throw new Error(
          `Enter a max heart rate between ${MIN_MAX_HEART_RATE_BPM} and ${MAX_MAX_HEART_RATE_BPM} bpm.`,
        );
      }

      setDraftHeartRateZoneBounds(
        zoneBoundsToDraft(
          calculateHeartRateZoneBoundsFromMaxHeartRate(parsedMaxHeartRate),
        ),
      );
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : "Failed to calculate heart rate zones.",
      );
    }
  }

  async function handleStartStravaConnect() {
    try {
      const result = await startStravaConnectMutation.beginAsync();
      window.location.assign(result.authorization_url);
    } catch {
      // Mutation errors are surfaced by the app-level React Query handler.
    }
  }

  async function handleQueueStravaSync() {
    try {
      await queueStravaSyncMutation.queueAsync();
      toast.success("Strava sync queued.");
    } catch {
      // Mutation errors are surfaced by the app-level React Query handler.
    }
  }

  async function handleDisconnectStrava() {
    try {
      const result = await disconnectStravaMutation.disconnectAsync();
      toast.success(result.message);
    } catch {
      // Mutation errors are surfaced by the app-level React Query handler.
    }
  }

  async function handleCompleteGarminLink() {
    try {
      const result = await completeGarminLinkMutation.completeAsync(
        garminPairingCode.trim().toUpperCase(),
      );
      toast.success(result.message);
    } catch {
      // Mutation errors are surfaced by the app-level React Query handler.
    }
  }

  async function handleUnlinkGarminDevice(id: number) {
    try {
      const result = await unlinkGarminDeviceMutation.unlinkAsync(id);
      toast.success(result.message);
    } catch {
      // Mutation errors are surfaced by the app-level React Query handler.
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
  const garminDevices = garminDevicesQuery.data ?? [];

  if (
    preferencesQuery.isLoading ||
    stravaQuery.isLoading ||
    garminDevicesQuery.isLoading
  ) {
    return <LoadingCard />;
  }

  return (
    <div className="space-y-6">
      <div>
        <p className="text-sm uppercase tracking-[0.22em] text-base-content/50">
          Account
        </p>
        <div className="mt-2 flex items-center gap-2">
          <h1 className="text-4xl font-semibold">Preferences</h1>
          <InfoTooltip
            label="Preferences details"
            tip={ACCOUNT_PREFERENCES_HELP_TEXT}
          />
        </div>
      </div>

      <AppCard bodyClassName="gap-6">
        <CardHeader
          title="Strava integration"
          titleExtras={
            <InfoTooltip
              label="Strava integration details"
              tip={STRAVA_INTEGRATION_HELP_TEXT}
            />
          }
          actions={
            <span
              className={`badge ${stravaConnection.connected ? "badge-primary" : "badge-outline"}`}
            >
              {stravaConnection.connected ? "Connected" : "Not connected"}
            </span>
          }
        />

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
                    {stravaConnection.athlete_name ?? "Connected athlete"}
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
                      <span key={scope} className="badge badge-ghost badge-sm">
                        {scope}
                      </span>
                    ))}
                  </div>
                ) : null}

                <div className="rounded-box border border-base-300 bg-base-100 p-4">
                  <div className="font-medium text-base-content">
                    {formatStravaSyncStatus(stravaConnection.last_sync_status)}
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
                        stravaConnection.last_synced_activity_started_at ?? "",
                      )}
                    </div>
                  </div>
                </div>
              </div>
            ) : (
              <div className="space-y-3 leading-6">
                <p>
                  Bike requests Strava activity access and stores the refresh
                  token so new syncs can happen without asking you to reconnect
                  each time.
                </p>
                <p>
                  The first sync backfills your available activities and runs
                  them through Bike&apos;s existing dedupe and import pipeline.
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
              <div className="font-medium text-base-content">Sync behavior</div>
              <p className="mt-2 leading-6">
                Bike imports the detailed activity streams available through
                Strava's API, stores them as retained source files, then reuses
                the existing upload pipeline so dedupe and derived metrics stay
                in one place. FIT originals are preserved when they come from a
                direct file upload.
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
                {isStravaSyncPending || queueStravaSyncMutation.isPending
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
      </AppCard>

      <AppCard bodyClassName="gap-6">
        <CardHeader
          title="Strava sync history"
          description="Recent OAuth, webhook, sync, and disconnect events for this account."
        />

        <IntegrationEventFeed
          events={stravaEventsQuery.data ?? []}
          isLoading={stravaEventsQuery.isLoading}
          error={stravaEventsQuery.error}
          emptyMessage="No Strava history yet. Connect Strava or queue a sync to start recording events."
        />
      </AppCard>

      <AppCard bodyClassName="gap-6">
        <CardHeader
          title="Garmin IQ linking"
          description="On the watch, choose Link account to get a pairing code. Enter that code here to approve the device. Bike stores only hashed refresh and access secrets for Garmin IQ sync."
          actions={
            <span
              className={`badge ${garminDevices.length > 0 ? "badge-primary" : "badge-outline"}`}
            >
              {garminDevices.length > 0 ? "Linked" : "Not linked"}
            </span>
          }
        />

        <div className="rounded-box border border-base-300 bg-base-200 p-4 text-sm text-base-content/75">
          <div className="font-medium text-base-content">
            Approve watch link
          </div>
          <p className="mt-2 leading-6">
            The watch polls until you approve. Once linked, the watch stores
            refresh credentials and rotates short-lived access tokens
            automatically.
          </p>

          <div className="mt-4 flex flex-col gap-3 sm:flex-row sm:items-end">
            <label className="form-control flex-1">
              <div className="label">
                <span className="label-text font-medium">Pairing code</span>
                <span className="label-text-alt">From watch</span>
              </div>
              <input
                type="text"
                className="input input-bordered uppercase"
                placeholder="A1B2C3"
                value={garminPairingCode}
                onChange={(event) => {
                  setGarminPairingCode(event.target.value.toUpperCase());
                }}
              />
            </label>

            <button
              type="button"
              className="btn btn-primary sm:mb-1"
              disabled={
                !garminPairingCode.trim() ||
                completeGarminLinkMutation.isPending
              }
              onClick={handleCompleteGarminLink}
            >
              {completeGarminLinkMutation.isPending
                ? "Approving..."
                : "Approve link"}
            </button>
          </div>
        </div>

        <div>
          <div className="text-sm font-medium text-base-content">
            Linked devices
          </div>

          {garminDevices.length === 0 ? (
            <div className="mt-3 rounded-box border border-base-300 bg-base-200 p-4 text-sm text-base-content/70">
              No Garmin IQ devices linked yet.
            </div>
          ) : (
            <div className="mt-3 space-y-3">
              {garminDevices.map((device) => (
                <div
                  key={device.id}
                  className="rounded-box border border-base-300 bg-base-200 p-4"
                >
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="text-sm text-base-content/75">
                      <div className="font-medium text-base-content">
                        {device.device_name?.trim() || "Garmin device"}
                      </div>
                      <div className="mt-1">Install: {device.install_id}</div>
                      <div>
                        Linked:{" "}
                        {formatActivityTimestamp(device.linked_at ?? "")}
                      </div>
                      <div>
                        Last seen:{" "}
                        {formatActivityTimestamp(device.last_seen_at ?? "")}
                      </div>
                    </div>

                    <button
                      type="button"
                      className="btn btn-ghost btn-sm"
                      disabled={unlinkGarminDeviceMutation.isPending}
                      onClick={() => {
                        void handleUnlinkGarminDevice(device.id);
                      }}
                    >
                      {unlinkGarminDeviceMutation.isPending
                        ? "Unlinking..."
                        : "Unlink"}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="card-actions justify-end gap-3 text-xs text-base-content/60">
          <span>
            If the code expires, restart linking on the watch and enter the new
            code.
          </span>
        </div>
      </AppCard>

      <AppCard bodyClassName="gap-6">
        <CardHeader
          title="Units"
          titleExtras={
            <InfoTooltip label="Units details" tip={UNITS_HELP_TEXT} />
          }
        />

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
      </AppCard>

      <AppCard bodyClassName="gap-6">
        <CardHeader
          title="Training profile"
          titleExtras={
            <InfoTooltip
              label="Training profile details"
              tip={TRAINING_PROFILE_HELP_TEXT}
            />
          }
        />

        {!heartRateZonesConfigured ? (
          <div className="alert alert-warning text-sm">
            <span>
              XC training needs heart rate zones for Z2 speed, decoupling, and
              weekly endurance load. Calculate them from max heart rate below or
              enter the zone ceilings manually.
            </span>
          </div>
        ) : null}

        <div className="grid gap-4 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,0.9fr)_minmax(0,1.3fr)]">
          <label className="form-control">
            <div className="label">
              <span className="label-text font-medium">Estimated FTP</span>
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
                Stored now for power zones and future load estimates once Bike
                carries power data.
              </span>
            </div>
          </label>

          <div className="rounded-box border border-base-300 bg-base-200/70 p-4 text-sm text-base-content/75">
            <div className="flex items-start justify-between gap-3">
              <div>
                <div className="font-medium text-base-content">
                  Calculate from max heart rate
                </div>
                <p className="mt-1 leading-6">
                  Bike can seed Z1 through Z4 ceilings from a simple HRmax
                  model, then you can edit the numbers manually before saving.
                </p>
              </div>
              <span className="badge badge-outline">Calculator</span>
            </div>

            <div className="mt-4 flex flex-col gap-3 sm:flex-row sm:items-end">
              <label className="form-control flex-1">
                <div className="label">
                  <span className="label-text font-medium">Max heart rate</span>
                  <span className="label-text-alt">Optional</span>
                </div>
                <input
                  type="number"
                  min={MIN_MAX_HEART_RATE_BPM}
                  max={MAX_MAX_HEART_RATE_BPM}
                  className="input input-bordered"
                  placeholder="182"
                  value={draftMaxHeartRate}
                  onChange={(event) => {
                    setDraftMaxHeartRate(event.target.value);
                  }}
                />
              </label>

              <button
                type="button"
                className="btn btn-outline sm:mb-1"
                onClick={handleCalculateHeartRateZones}
              >
                Calculate zones
              </button>
            </div>

            <p className="mt-3 text-xs leading-6 text-base-content/60">
              Uses simple zone ceilings at 60%, 70%, 80%, and 90% of max heart
              rate. Adjust them manually if you use a more specific
              threshold-based setup.
            </p>
          </div>

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
                <span className="label-text font-medium">{field.label}</span>
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
          <div className="flex items-center gap-2">
            <div className="font-medium text-base-content">
              How Bike uses this
            </div>
            <InfoTooltip
              label="Training profile usage details"
              tip={TRAINING_PROFILE_USAGE_HELP_TEXT}
            />
          </div>
        </div>

        <div className="card-actions justify-end">
          <button
            type="button"
            className="btn btn-ghost"
            disabled={!isDirty || updatePreferencesMutation.isPending}
            onClick={() => {
              setDraftUnitSystem(unitSystem);
              setDraftEstimatedFtpWatts(estimatedFtpWatts?.toString() ?? "");
              setDraftMaxHeartRate("");
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
      </AppCard>
    </div>
  );
}
