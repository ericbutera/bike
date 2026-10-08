"use client";

import dynamic from "next/dynamic";
import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { $typedApi } from "../lib/api";
import { useAuth } from "../lib/auth";
import { ACTIVITY_SPORT_OPTIONS } from "../lib/activitySports";
import { FLAG_HEATMAPS } from "../lib/featureFlags";
import { usePublicFeatureFlags } from "../lib/queries";
import {
  heatmapPreset,
  heatmapTileUrl,
  parseHeatmapFilters,
} from "../lib/heatmaps";
import { useDocumentTitle } from "../lib/useDocumentTitle";
import {
  HEATMAP_COLOR_STORAGE_KEY,
  heatmapPalette,
  heatmapPaletteColor,
  type HeatmapPaletteId,
} from "../lib/heatmapColors";
import HeatmapColorPicker from "./HeatmapColorPicker";
import HeatmapHelp from "./HeatmapHelp";
import HeatmapZoomPresets from "./HeatmapZoomPresets";
import {
  parseHeatmapCamera,
  type HeatmapCamera,
  type HeatmapViewAction,
} from "../lib/heatmapCamera";

const HeatmapMap = dynamic(() => import("./HeatmapMap"), {
  ssr: false,
  loading: () => <div className="absolute inset-0 animate-pulse bg-base-300" />,
});
const HEATMAP_PALETTE_CHANGE_EVENT = "bike:heatmap-palette-change";

function subscribeToNothing() {
  return () => {};
}

function getBrowserTimeZone() {
  return Intl.DateTimeFormat().resolvedOptions().timeZone;
}

function getStoredPaletteId(): HeatmapPaletteId {
  try {
    return heatmapPalette(
      window.localStorage.getItem(HEATMAP_COLOR_STORAGE_KEY),
    ).id;
  } catch {
    return "blue";
  }
}

function subscribeToPalette(onChange: () => void) {
  window.addEventListener("storage", onChange);
  window.addEventListener(HEATMAP_PALETTE_CHANGE_EVENT, onChange);
  return () => {
    window.removeEventListener("storage", onChange);
    window.removeEventListener(HEATMAP_PALETTE_CHANGE_EVENT, onChange);
  };
}

export default function HeatmapPanel() {
  useDocumentTitle("Maps");
  const { user } = useAuth();
  const flags = usePublicFeatureFlags();
  const enabled =
    !flags.isPending &&
    !flags.isError &&
    flags.data.some(
      (flag) => flag.feature_key === FLAG_HEATMAPS && flag.enabled,
    );
  const router = useRouter();
  const search = useSearchParams();
  const browserTimeZone = useSyncExternalStore(
    subscribeToNothing,
    getBrowserTimeZone,
    () => null,
  );
  const [tileError, setTileError] = useState<string>();
  const [locationError, setLocationError] = useState<string>();
  const [viewRequest, setViewRequest] = useState<{
    action: HeatmapViewAction;
    sequence: number;
  }>();
  const requestView = (action: HeatmapViewAction) => {
    setLocationError(undefined);
    setViewRequest((previous) => ({
      action,
      sequence: (previous?.sequence ?? 0) + 1,
    }));
  };
  const [refreshKey, setRefreshKey] = useState(0);
  const storedPaletteId = useSyncExternalStore(
    subscribeToPalette,
    getStoredPaletteId,
    () => "blue" as HeatmapPaletteId,
  );
  const [unavailableStoragePalette, setUnavailableStoragePalette] =
    useState<HeatmapPaletteId | null>(null);
  const paletteId = unavailableStoragePalette ?? storedPaletteId;
  const refreshing = useRef(false);
  const lastStaleRevision = useRef<string | undefined>(undefined);
  const choosePalette = (value: HeatmapPaletteId) => {
    try {
      window.localStorage.setItem(HEATMAP_COLOR_STORAGE_KEY, value);
      setUnavailableStoragePalette(null);
      window.dispatchEvent(new Event(HEATMAP_PALETTE_CHANGE_EVENT));
    } catch {
      setUnavailableStoragePalette(value);
    }
  };
  const lineColor = heatmapPaletteColor(heatmapPalette(paletteId));
  const filters = parseHeatmapFilters(
    new URLSearchParams(search.toString()),
    browserTimeZone ?? "UTC",
  );
  const queryOptions = $typedApi.queryOptions("get", "/maps/heatmap", {
    params: { query: filters.query },
  });
  const response = $typedApi.useQuery(
    "get",
    "/maps/heatmap",
    { params: { query: filters.query } },
    {
      queryKey: [...queryOptions.queryKey, user?.id],
      enabled: Boolean(user && enabled && browserTimeZone && !filters.error),
      retry: false,
      refetchInterval: (query) => (query.state.data?.preparing ? 5000 : 30000),
    },
  );
  const metadata = enabled && !filters.error ? response.data : undefined;
  const zonesQueryOptions = $typedApi.queryOptions(
    "get",
    "/maps/heatmap/zones",
    {
      params: {
        query: { ...filters.query, revision: metadata?.revision },
      },
    },
  );
  const zonesResponse = $typedApi.useQuery(
    "get",
    "/maps/heatmap/zones",
    { params: { query: { ...filters.query, revision: metadata?.revision } } },
    {
      queryKey: [...zonesQueryOptions.queryKey, user?.id],
      enabled: Boolean(user && enabled && metadata && !filters.error),
      retry: false,
      staleTime: 5 * 60 * 1000,
    },
  );
  const update = (values: Record<string, string>) => {
    const params = new URLSearchParams(search.toString());
    params.set("tz", filters.timeZone);
    for (const [key, value] of Object.entries(values)) {
      if (value) params.set(key, value);
      else params.delete(key);
    }
    setTileError(undefined);
    router.replace(`/maps?${params}`, { scroll: false });
  };
  const saveCamera = (camera: HeatmapCamera) => {
    const params = new URLSearchParams(window.location.search);
    params.set("lng", String(camera.longitude));
    params.set("lat", String(camera.latitude));
    params.set("zoom", String(camera.zoom));
    if (
      params.toString() ===
      new URLSearchParams(window.location.search).toString()
    )
      return;
    window.history.replaceState(null, "", `/maps?${params}`);
  };
  const refresh = async () => {
    if (refreshing.current) return;
    refreshing.current = true;
    setTileError(undefined);
    try {
      await response.refetch();
      setRefreshKey((key) => key + 1);
    } finally {
      refreshing.current = false;
    }
  };
  const stale = () => {
    if (lastStaleRevision.current === metadata?.revision) return;
    lastStaleRevision.current = metadata?.revision;
    void refresh();
  };
  if (flags.isPending)
    return (
      <div className="grid h-full place-items-center">
        <p role="status" className="rounded-lg bg-base-100/90 px-4 py-3 shadow">
          Checking heatmap availability…
        </p>
      </div>
    );
  if (flags.isError)
    return (
      <div className="grid h-full place-items-center p-4">
        <div role="alert" className="alert alert-error max-w-lg shadow-lg">
          <span>Unable to check heatmap availability.</span>
          <button className="btn btn-sm" onClick={() => void flags.refetch()}>
            Retry
          </button>
        </div>
      </div>
    );
  if (!enabled)
    return (
      <div className="grid h-full place-items-center p-4">
        <section className="rounded-xl border border-base-300 bg-base-100 p-6 shadow-lg">
          <h1 className="text-2xl font-semibold">Maps</h1>
          <p className="mt-3">Heatmaps are not enabled on this site.</p>
        </section>
      </div>
    );

  return (
    <section className="relative h-full w-full overflow-hidden bg-base-200">
      <HeatmapMap
        key={String(user?.id)}
        metadata={metadata}
        zones={
          metadata && zonesResponse.data?.revision === metadata.revision
            ? zonesResponse.data.zones
            : []
        }
        tileUrl={
          metadata?.ready
            ? `${heatmapTileUrl(metadata)}&refresh=${refreshKey}`
            : undefined
        }
        paletteId={paletteId}
        onStale={stale}
        onTileError={setTileError}
        view={{
          camera: parseHeatmapCamera(new URLSearchParams(search.toString())),
          request: viewRequest,
          onChange: saveCamera,
          onRequest: requestView,
          onError: setLocationError,
        }}
        onMapMoveStart={() => setTileError(undefined)}
      />
      <div className="pointer-events-none absolute right-0 top-0 z-20 flex max-w-full flex-col items-end gap-2 p-3 sm:p-4">
        <div
          role="toolbar"
          aria-label="Heatmap controls"
          className="pointer-events-auto flex max-w-full flex-wrap items-center justify-end gap-1 rounded-xl border border-base-300/70 bg-base-100/95 p-2 shadow-lg backdrop-blur"
        >
          <HeatmapZoomPresets
            onChoose={requestView}
            canFit={Boolean(metadata?.bounds)}
          />
          <HeatmapColorPicker value={paletteId} onChange={choosePalette} />
          <details className="dropdown dropdown-end">
            <summary className="btn btn-sm btn-ghost">Filters</summary>
            <div className="dropdown-content z-30 mt-2 w-[min(94vw,56rem)] rounded-box border border-base-300 bg-base-100 p-4 shadow-xl">
              <div className="flex flex-wrap items-end gap-3">
                <label className="flex flex-col gap-1 text-sm">
                  Activity type
                  <select
                    className="select select-bordered select-sm"
                    value={filters.sport}
                    onChange={(event) => update({ sport: event.target.value })}
                  >
                    <option value="">All activities</option>
                    {ACTIVITY_SPORT_OPTIONS.filter(
                      (sport) => sport.value !== "indoor_trainer_ride",
                    ).map((sport) => (
                      <option key={sport.value} value={sport.value}>
                        {sport.label}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="flex flex-col gap-1 text-sm">
                  From
                  <input
                    className="input input-bordered input-sm"
                    type="date"
                    value={filters.start}
                    onChange={(event) => update({ start: event.target.value })}
                  />
                </label>
                <label className="flex flex-col gap-1 text-sm">
                  Through
                  <input
                    className="input input-bordered input-sm"
                    type="date"
                    value={filters.end}
                    onChange={(event) => update({ end: event.target.value })}
                  />
                </label>
                <div className="flex flex-wrap gap-1">
                  {(
                    [
                      { key: "all", label: "All time" },
                      { key: "year", label: "This year" },
                      { key: "month", label: "Last 30 days" },
                    ] as const
                  ).map((preset) => (
                    <button
                      key={preset.key}
                      className="btn btn-sm btn-ghost"
                      disabled={filters.error === "Choose a valid timezone."}
                      onClick={() =>
                        update(heatmapPreset(preset.key, filters.timeZone))
                      }
                    >
                      {preset.label}
                    </button>
                  ))}
                </div>
                <p className="text-xs opacity-60">
                  Dates use {filters.timeZone}.
                </p>
              </div>
            </div>
          </details>
          <HeatmapHelp metadata={metadata} color={lineColor} />
        </div>
        {((response.isFetching && !metadata) ||
          metadata?.preparing ||
          Boolean(metadata?.failed)) && (
          <p
            role="status"
            className="rounded-lg bg-base-100/95 px-3 py-2 text-sm shadow-lg"
          >
            {response.isFetching && !metadata ? "Loading your routes…" : ""}
            {metadata?.preparing
              ? `Preparing ${metadata.pending.toLocaleString()} activities…`
              : ""}
            {metadata?.failed
              ? ` ${metadata.failed} routes could not be prepared`
              : ""}
          </p>
        )}
      </div>
      {metadata?.ready === 0 && (
        <div className="pointer-events-none absolute inset-x-4 top-1/2 z-10 -translate-y-1/2 text-center">
          <p className="pointer-events-auto mx-auto max-w-lg rounded-xl border border-base-300/70 bg-base-100/95 p-4 shadow-lg backdrop-blur">
            {metadata.preparing
              ? "Your history is being prepared. Routes will appear as they are ready."
              : "No recorded outdoor routes match these filters."}
          </p>
        </div>
      )}
      <div className="pointer-events-none absolute bottom-3 left-3 z-10 flex max-w-[min(36rem,calc(100vw-1.5rem))] flex-col gap-2 sm:bottom-4 sm:left-4">
        {filters.error && (
          <div className="pointer-events-auto flex flex-wrap items-center gap-3 rounded-lg bg-base-100/95 p-3 shadow-lg backdrop-blur">
            <p role="alert" className="text-sm text-error">
              {filters.error}
            </p>
            <button
              className="btn btn-sm"
              onClick={() =>
                update({
                  start: "",
                  end: "",
                  sport: "",
                  tz: browserTimeZone ?? "UTC",
                })
              }
            >
              Reset filters
            </button>
          </div>
        )}
        {(response.isError || tileError) && (
          <p
            role="alert"
            className="pointer-events-auto rounded-lg bg-base-100/95 p-3 text-sm text-error shadow-lg backdrop-blur"
          >
            {tileError ?? "Could not load your heatmap. Try again later."}
          </p>
        )}
        {locationError && (
          <p
            role="alert"
            className="pointer-events-auto rounded-lg bg-base-100/95 p-3 text-sm text-error shadow-lg backdrop-blur"
          >
            {locationError}
          </p>
        )}
      </div>
    </section>
  );
}
