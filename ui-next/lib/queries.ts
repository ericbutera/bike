import { useQueryClient } from "@tanstack/react-query";
import { $api } from "./api";

export type PaginationMetadata = {
  page: number;
  per_page: number;
  total: number;
  total_pages: number;
};

export type PaginatedResponse<T> = {
  data: T[];
  metadata: PaginationMetadata;
};

export type ActivityLap = {
  lap_index: number;
  title: string;
  start_offset_seconds?: number | null;
  duration_seconds?: number | null;
  distance_meters?: number | null;
  elevation_gain_meters?: number | null;
  elevation_loss_meters?: number | null;
  average_speed_mps?: number | null;
  max_speed_mps?: number | null;
  average_heart_rate_bpm?: number | null;
  max_heart_rate_bpm?: number | null;
  average_cadence_rpm?: number | null;
  max_cadence_rpm?: number | null;
  calories?: number | null;
};

export type ActivityChartPoint = {
  elapsed_seconds: number;
  distance_meters?: number | null;
  elevation_meters?: number | null;
  speed_mps?: number | null;
  heart_rate_bpm?: number | null;
  cadence_rpm?: number | null;
};

export type ActivityRoutePoint = {
  elapsed_seconds: number;
  latitude: number;
  longitude: number;
  distance_meters?: number | null;
  elevation_meters?: number | null;
  speed_mps?: number | null;
  heart_rate_bpm?: number | null;
  cadence_rpm?: number | null;
};

export type ActivitySegmentEffort = {
  segment_id: number;
  segment_title: string;
  effort_index: number;
  duration_seconds: number;
  start_route_point_index: number;
  end_route_point_index: number;
  overall_rank?: number | null;
  personal_rank?: number | null;
  personal_best_duration_seconds?: number | null;
};

export type ActivityHeartRateZone = {
  zone: number;
  label: string;
  min_bpm?: number | null;
  max_bpm?: number | null;
  duration_seconds: number;
  share_percent: number;
};

export type Activity = {
  id: number;
  title: string;
  sport: string;
  source: string;
  original_filename?: string | null;
  format?: string | null;
  started_at: string;
  ended_at?: string | null;
  location?: string | null;
  distance_meters?: number | null;
  moving_time_seconds?: number | null;
  total_time_seconds?: number | null;
  elevation_gain_meters?: number | null;
  elevation_loss_meters?: number | null;
  average_speed_mps?: number | null;
  max_speed_mps?: number | null;
  average_heart_rate_bpm?: number | null;
  max_heart_rate_bpm?: number | null;
  average_cadence_rpm?: number | null;
  max_cadence_rpm?: number | null;
  calories?: number | null;
  estimated_ftp_watts?: number | null;
  heart_rate_zones?: ActivityHeartRateZone[] | null;
  laps?: ActivityLap[] | null;
  chart_points?: ActivityChartPoint[] | null;
  route_points?: ActivityRoutePoint[] | null;
  segment_efforts?: ActivitySegmentEffort[] | null;
  can_regenerate?: boolean;
};

export type SegmentEffort = {
  id: number;
  rider_user_id: number;
  activity_id: number;
  activity_title: string;
  rider_name: string;
  activity_started_at: string;
  effort_index: number;
  duration_seconds: number;
  start_elapsed_seconds: number;
  end_elapsed_seconds: number;
  distance_meters?: number | null;
  route_points?: ActivityRoutePoint[] | null;
};

export type Segment = {
  id: number;
  title: string;
  source: string;
  original_filename?: string | null;
  format?: string | null;
  distance_meters?: number | null;
  effort_count: number;
  best_duration_seconds?: number | null;
  current_user_pr_duration_seconds?: number | null;
  created_at: string;
  route_points?: ActivityRoutePoint[] | null;
  efforts?: SegmentEffort[] | null;
};

export type UserPreferences = {
  unit_system: string;
  estimated_ftp_watts?: number | null;
  heart_rate_zone_bounds_bpm?: number[] | null;
};

export type StravaConnection = {
  configured: boolean;
  connected: boolean;
  athlete_id?: number | null;
  athlete_name?: string | null;
  athlete_username?: string | null;
  athlete_profile_medium_url?: string | null;
  scopes: string[];
  last_sync_status: string;
  last_sync_message?: string | null;
  last_sync_started_at?: string | null;
  last_sync_finished_at?: string | null;
  last_synced_activity_started_at?: string | null;
  last_sync_imported_count: number;
  last_sync_duplicate_count: number;
  last_sync_failed_count: number;
};

export type FitnessFreshnessPoint = {
  date: string;
  training_load: number;
  fitness: number;
  fatigue: number;
  form: number;
};

export type FitnessFreshnessResponse = {
  start_date: string;
  end_date: string;
  fitness_window_days: number;
  fatigue_window_days: number;
  points: FitnessFreshnessPoint[];
};

export type AdminAnalyticsBackfillResponse = {
  user_count: number;
  segment_count: number;
  fitness_task_count: number;
  segment_task_count: number;
  total_tasks_enqueued: number;
  segment_chunk_size: number;
};

export type ActivityArchiveImportJob = {
  id: number;
  archive_url: string;
  resolved_url?: string | null;
  status: string;
  failure_message?: string | null;
  total_entries: number;
  supported_entry_count: number;
  imported_count: number;
  duplicate_count: number;
  skipped_unsupported_count: number;
  failed_count: number;
  error_samples: string[];
  created_at: string;
  started_at?: string | null;
  finished_at?: string | null;
  updated_at: string;
};

export type ActivityImport = {
  id: number;
  activity_id?: number | null;
  original_filename: string;
  format: string;
  status: string;
  size_bytes: number;
  mime_type?: string | null;
  created_at: string;
  activity_started_at?: string | null;
  activity_duration_seconds?: number | null;
  activity_location?: string | null;
};

export function useAdminMetrics() {
  return $api.useQuery("get", "/admin/metrics", {});
}

export function useAdminAppMetrics() {
  return $api.useQuery("get", "/admin/metrics/app", {});
}

export function useAdminBackfillAnalytics() {
  const mutation = $api.useMutation("post", "/admin/analytics/backfill");

  return {
    ...mutation,
    backfillAsync: async () => {
      const result = await mutation.mutateAsync({});

      return result as AdminAnalyticsBackfillResponse;
    },
  };
}

export function useImportActivityArchiveUrl() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("post", "/activity-imports/archive-url");

  return {
    ...mutation,
    importAsync: async (archiveUrl: string) => {
      const result = await mutation.mutateAsync({
        body: {
          archive_url: archiveUrl,
        },
      });

      await queryClient.invalidateQueries({
        queryKey: ["get", "/activity-imports/archive-jobs"],
      });

      return result as ActivityArchiveImportJob;
    },
  };
}

export function useActivities(opts?: {
  enabled?: boolean;
  page?: number;
  perPage?: number;
}) {
  const page = Math.max(1, opts?.page ?? 1);
  const perPage = Math.max(1, opts?.perPage ?? 10);
  const response = $api.useQuery("get", "/activities", {
    params: { query: { page, per_page: perPage } },
    options: { enabled: opts?.enabled ?? true },
  });

  const pageData = (response.data ?? {
    data: [],
    metadata: {
      page,
      per_page: perPage,
      total: 0,
      total_pages: 1,
    },
  }) as PaginatedResponse<Activity>;

  return {
    ...response,
    data: pageData.data,
    metadata: pageData.metadata,
  };
}

export function useActivity(id: number | string | null | undefined) {
  const numericId = Number(id);
  const enabled = Number.isFinite(numericId) && numericId > 0;
  const response = $api.useQuery("get", "/activities/{id}", {
    params: { path: { id: enabled ? numericId : 0 } },
    options: { enabled },
  });

  return {
    ...response,
    data: (response.data ?? null) as Activity | null,
  };
}

export function useRegenerateActivity() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("post", "/activities/{id}/regenerate");

  return {
    ...mutation,
    regenerateAsync: async (id: number | string) => {
      const numericId = Number(id);
      const result = await mutation.mutateAsync({
        params: { path: { id: numericId } },
      });

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["get", "/activities"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/activities/{id}"],
        }),
        queryClient.invalidateQueries({ queryKey: ["get", "/segments"] }),
        queryClient.invalidateQueries({ queryKey: ["get", "/segments/{id}"] }),
      ]);

      return result as Activity;
    },
  };
}

export function useDeleteActivity() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("delete", "/activities/{id}");

  return {
    ...mutation,
    deleteAsync: async (id: number | string) => {
      const numericId = Number(id);
      const result = await mutation.mutateAsync({
        params: { path: { id: numericId } },
      });

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["get", "/activities"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/activities/{id}"],
        }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/activity-imports"],
        }),
        queryClient.invalidateQueries({ queryKey: ["get", "/segments"] }),
        queryClient.invalidateQueries({ queryKey: ["get", "/segments/{id}"] }),
      ]);

      return result;
    },
  };
}

export function useSegments(opts?: { enabled?: boolean }) {
  const response = $api.useQuery("get", "/segments", {
    options: { enabled: opts?.enabled ?? true },
  });

  return {
    ...response,
    data: (response.data ?? []) as Segment[],
  };
}

export function useSegment(id: number | string | null | undefined) {
  const numericId = Number(id);
  const enabled = Number.isFinite(numericId) && numericId > 0;
  const response = $api.useQuery("get", "/segments/{id}", {
    params: { path: { id: enabled ? numericId : 0 } },
    options: { enabled },
  });

  return {
    ...response,
    data: (response.data ?? null) as Segment | null,
  };
}

export function useUploadSegment() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("post", "/segments");

  return {
    ...mutation,
    uploadAsync: async (file: File) => {
      const form = new FormData();
      form.append("file", file);

      const result = await mutation.mutateAsync({ body: form });

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["get", "/segments"] }),
        queryClient.invalidateQueries({ queryKey: ["get", "/segments/{id}"] }),
        queryClient.invalidateQueries({ queryKey: ["get", "/activities"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/activities/{id}"],
        }),
      ]);

      return result as Segment;
    },
  };
}

export function useActivityImports(opts?: {
  enabled?: boolean;
  refetchIntervalMs?: number | false;
}) {
  const response = $api.useQuery("get", "/activity-imports", {
    options: {
      enabled: opts?.enabled ?? true,
      refetchInterval: opts?.refetchIntervalMs ?? false,
    },
  });

  return {
    ...response,
    data: (response.data ?? []) as ActivityImport[],
  };
}

export function useActivityArchiveImportJobs(opts?: {
  enabled?: boolean;
  refetchIntervalMs?: number | false;
}) {
  const response = $api.useQuery("get", "/activity-imports/archive-jobs", {
    options: {
      enabled: opts?.enabled ?? true,
      refetchInterval: opts?.refetchIntervalMs ?? false,
    },
  });

  return {
    ...response,
    data: (response.data ?? []) as ActivityArchiveImportJob[],
  };
}

export function useUploadActivityImport() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("post", "/activity-imports");

  return {
    ...mutation,
    uploadAsync: async (file: File) => {
      const form = new FormData();
      form.append("file", file);

      const result = await mutation.mutateAsync({ body: form });

      queryClient.invalidateQueries({ queryKey: ["get", "/activities"] });
      queryClient.invalidateQueries({ queryKey: ["get", "/activity-imports"] });

      return result as ActivityImport;
    },
  };
}

export function useStravaConnection(opts?: {
  enabled?: boolean;
  refetchIntervalMs?: number | false;
}) {
  const response = $api.useQuery("get", "/strava/connection", {
    options: {
      enabled: opts?.enabled ?? true,
      refetchInterval: opts?.refetchIntervalMs ?? false,
    },
  });

  return {
    ...response,
    data: (response.data ?? {
      configured: false,
      connected: false,
      athlete_id: null,
      athlete_name: null,
      athlete_username: null,
      athlete_profile_medium_url: null,
      scopes: [],
      last_sync_status: "never",
      last_sync_message: null,
      last_sync_started_at: null,
      last_sync_finished_at: null,
      last_synced_activity_started_at: null,
      last_sync_imported_count: 0,
      last_sync_duplicate_count: 0,
      last_sync_failed_count: 0,
    }) as StravaConnection,
  };
}

export function useStartStravaConnect() {
  const mutation = $api.useMutation("post", "/strava/connect");

  return {
    ...mutation,
    beginAsync: async () => {
      const result = await mutation.mutateAsync({});

      return result as { authorization_url: string };
    },
  };
}

export function useQueueStravaSync() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("post", "/strava/sync");

  return {
    ...mutation,
    queueAsync: async () => {
      const result = await mutation.mutateAsync({});

      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: ["get", "/strava/connection"],
        }),
        queryClient.invalidateQueries({ queryKey: ["get", "/activities"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/activity-imports"],
        }),
      ]);

      return result as StravaConnection;
    },
  };
}

export function useDisconnectStrava() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("delete", "/strava/connection");

  return {
    ...mutation,
    disconnectAsync: async () => {
      const result = await mutation.mutateAsync({});

      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: ["get", "/strava/connection"],
        }),
        queryClient.invalidateQueries({ queryKey: ["get", "/activities"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/activity-imports"],
        }),
      ]);

      return result as { message: string };
    },
  };
}

export function useUserPreferences(opts?: { enabled?: boolean }) {
  const response = $api.useQuery("get", "/preferences", {
    options: { enabled: opts?.enabled ?? true },
  });

  return {
    ...response,
    data: (response.data ?? {
      unit_system: "mixed",
      estimated_ftp_watts: null,
      heart_rate_zone_bounds_bpm: null,
    }) as UserPreferences,
  };
}

export function useFitnessFreshness(opts?: {
  enabled?: boolean;
  startDate?: string;
  endDate?: string;
}) {
  const response = $api.useQuery("get", "/fitness", {
    params: {
      query: {
        start_date: opts?.startDate,
        end_date: opts?.endDate,
      },
    },
    options: { enabled: opts?.enabled ?? true },
  });

  return {
    ...response,
    data: (response.data ?? null) as FitnessFreshnessResponse | null,
  };
}

export function useUpdateUserPreferences() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("put", "/preferences");

  return {
    ...mutation,
    updateAsync: async (preferences: UserPreferences) => {
      const result = await mutation.mutateAsync({ body: preferences });

      await queryClient.invalidateQueries({
        queryKey: ["get", "/preferences"],
      });

      return result as UserPreferences;
    },
  };
}
