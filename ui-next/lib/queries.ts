import { useQueryClient } from "@tanstack/react-query";
import type { ActivityType } from "./activityTypes";
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
  power_watts?: number | null;
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
  power_watts?: number | null;
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

export type ActivityAchievementHighlight = {
  segment_id: number;
  segment_title: string;
  effort_index: number;
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

export type ActivityRideFocus =
  | "xc_endurance"
  | "mixed_xc"
  | "dh_session"
  | "other";

export type ActivityTrainingAnalysis = {
  ride_focus: ActivityRideFocus;
  route_family_key?: string | null;
  comparable_distance_bucket_meters?: number | null;
  comparable_elevation_gain_bucket_meters?: number | null;
  aerobic_decoupling_percent?: number | null;
  z2_time_seconds: number;
  z2_distance_meters?: number | null;
  z2_average_speed_mps?: number | null;
  climbing_time_seconds: number;
  climbing_elevation_gain_meters?: number | null;
  sustained_climb_count: number;
};

export type Activity = {
  id: number;
  title: string;
  sport: string;
  source: string;
  activity_type?: ActivityType;
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
  relative_effort?: number | null;
  estimated_ftp_watts?: number | null;
  heart_rate_zones?: ActivityHeartRateZone[] | null;
  laps?: ActivityLap[] | null;
  chart_points?: ActivityChartPoint[] | null;
  route_points?: ActivityRoutePoint[] | null;
  achievement_highlights?: ActivityAchievementHighlight[] | null;
  segment_efforts?: ActivitySegmentEffort[] | null;
  training_analysis?: ActivityTrainingAnalysis | null;
  can_regenerate?: boolean;
};

export type UpdateActivityInput = {
  activity_type?: ActivityType | null;
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

export type SegmentBuilderSource = {
  activity_id: number;
  start_route_point_index: number;
  end_route_point_index: number;
};

export type SegmentMode = "xc" | "dh";

export type Segment = {
  id: number;
  title: string;
  source: string;
  mode: SegmentMode;
  starred?: boolean;
  original_filename?: string | null;
  format?: string | null;
  distance_meters?: number | null;
  effort_count: number;
  best_duration_seconds?: number | null;
  current_user_pr_duration_seconds?: number | null;
  created_at: string;
  processing_task_id?: string | null;
  processing_task_status?: string | null;
  builder_source?: SegmentBuilderSource | null;
  route_points?: ActivityRoutePoint[] | null;
  efforts?: SegmentEffort[] | null;
};

export type SegmentComparison = {
  segment_id: number;
  route_points?: ActivityRoutePoint[] | null;
  efforts?: SegmentEffort[] | null;
};

export type UserPreferences = {
  unit_system: string;
  estimated_ftp_watts?: number | null;
  heart_rate_zone_bounds_bpm?: number[] | null;
  xc_goal_event_name?: string | null;
  xc_goal_start_date?: string | null;
  xc_goal_target_date?: string | null;
  xc_goal_target_distance_meters?: number | null;
  xc_goal_target_elevation_gain_meters?: number | null;
  xc_goal_target_finish_time_seconds?: number | null;
  xc_goal_event_profile?: XcEventProfile | null;
  xc_goal_backfill_status?: string | null;
  xc_goal_backfill_completed_at?: string | null;
};

export type ActivityProcessingState = {
  is_active: boolean;
  source?: string | null;
  source_label?: string | null;
  stage?: string | null;
  stage_label?: string | null;
  message?: string | null;
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

export type GarminIqCompleteLinkResponse = {
  message: string;
  install_id: string;
  device_name?: string | null;
};

export type GarminIqLinkedDevice = {
  id: number;
  install_id: string;
  device_name?: string | null;
  linked_at?: string | null;
  last_seen_at?: string | null;
};

export type IntegrationEvent = {
  id: number;
  user_id?: number | null;
  provider: string;
  event_type: string;
  level: string;
  message: string;
  connection_id?: number | null;
  payload?: Record<string, unknown> | null;
  created_at: string;
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

export type TrainingGoalKey =
  | "weekly_z2_average"
  | "weekly_climbing_average"
  | "aerobic_decoupling"
  | "dh_laps_per_session"
  | "dh_repeat_fade"
  | "dh_rolling_top3_gap";

export type TrainingMetricUnit =
  | "seconds"
  | "meters"
  | "percent"
  | "count"
  | "meters_per_second"
  | "meters_per_kilometer"
  | "meters_per_hour";

export type TrainingGoalDirection = "at_least" | "at_most";

export type TrainingRecommendationKey =
  | "build_xc_baseline"
  | "repeat_comparable_endurance_ride"
  | "increase_endurance_volume"
  | "add_climbing_endurance"
  | "hold_steady_endurance"
  | "maintain_endurance_rhythm"
  | "recover_before_next_xc_ride"
  | "use_positive_form_for_xc_benchmark"
  | "mark_dh_segments"
  | "add_dh_repeats"
  | "reduce_dh_fade"
  | "chase_dh_consistency"
  | "maintain_dh_momentum"
  | "recover_before_next_dh_session"
  | "use_positive_form_for_dh_benchmark";

export type TrainingRecommendationPriority = "high" | "medium" | "low";

export type XcEventProfile =
  | "xc_marathon"
  | "technical_singletrack"
  | "endurance_mtb"
  | "ultra_mtb"
  | "custom";

export type XcReadinessStatus =
  | "on_track"
  | "watch"
  | "falling_behind"
  | "missing_data";

export type XcReadinessGateKey =
  | "long_ride_distance"
  | "big_climb_day"
  | "climb_density"
  | "target_finish_pace"
  | "aerobic_decoupling"
  | "recovery";

export type XcTrainingDeficitKey =
  | "long_ride"
  | "big_climb_day"
  | "event_specificity"
  | "finish_pace"
  | "aerobic_durability"
  | "recovery";

export type XcTrainingPurpose =
  | "base_endurance"
  | "climb_durability"
  | "tempo"
  | "threshold"
  | "punch_vo2"
  | "technical_fatigue"
  | "recovery"
  | "data_quality";

export type XcSuggestedRide = {
  purpose: XcTrainingPurpose;
  duration_seconds_min?: number | null;
  duration_seconds_max?: number | null;
  distance_meters_min?: number | null;
  distance_meters_max?: number | null;
  climbing_elevation_gain_meters?: number | null;
  intensity: string;
  terrain: string;
  detail: string;
};

export type TrainingGoalMetric = {
  key: TrainingGoalKey;
  label: string;
  unit: TrainingMetricUnit;
  direction: TrainingGoalDirection;
  current_value?: number | null;
  target_value: number;
  progress_percent?: number | null;
};

export type TrainingRecommendation = {
  key: TrainingRecommendationKey;
  priority: TrainingRecommendationPriority;
  title: string;
  detail: string;
  purpose?: XcTrainingPurpose | null;
  limiter?: string | null;
  gap_value?: number | null;
  gap_unit?: TrainingMetricUnit | null;
  suggested_ride?: XcSuggestedRide | null;
};

export type XcProgressSummary = {
  recent_window_days: number;
  recent_ride_count: number;
  comparable_ride_count: number;
  total_z2_time_seconds: number;
  total_climbing_time_seconds: number;
  total_climbing_elevation_gain_meters: number;
  average_aerobic_decoupling_percent?: number | null;
};

export type XcRideProgress = {
  activity_id: number;
  activity_title: string;
  started_at: string;
  activity_type: ActivityType;
  ride_focus: ActivityRideFocus;
  route_family_key?: string | null;
  distance_meters?: number | null;
  elevation_gain_meters?: number | null;
  moving_time_seconds?: number | null;
  z2_time_seconds: number;
  z2_distance_meters?: number | null;
  z2_average_speed_mps?: number | null;
  climbing_time_seconds: number;
  climbing_elevation_gain_meters?: number | null;
  aerobic_decoupling_percent?: number | null;
  z1_seconds: number;
  z2_zone_seconds: number;
  z3_seconds: number;
  z4_seconds: number;
  z5_seconds: number;
  training_purpose: XcTrainingPurpose;
  training_purpose_detail: string;
};

export type XcRaceResult = {
  activity_id: number;
  activity_title: string;
  started_at: string;
  distance_meters?: number | null;
  elevation_gain_meters?: number | null;
  moving_time_seconds?: number | null;
  average_speed_mps?: number | null;
  climb_density_meters_per_kilometer?: number | null;
  z2_time_seconds: number;
  climbing_time_seconds: number;
  climbing_elevation_gain_meters?: number | null;
  aerobic_decoupling_percent?: number | null;
  prior_training_ride_count: number;
  prior_training_z2_time_seconds: number;
  prior_training_climbing_elevation_gain_meters: number;
  prior_training_average_z2_speed_mps?: number | null;
  prior_training_average_aerobic_decoupling_percent?: number | null;
  race_vs_best_training_distance_percent?: number | null;
  race_vs_best_training_elevation_percent?: number | null;
  insight_title: string;
  insight_detail: string;
};

export type XcWeeklyProgressPoint = {
  week_start: string;
  ride_count: number;
  comparable_ride_count: number;
  distance_meters: number;
  z2_time_seconds: number;
  z2_distance_meters: number;
  average_z2_speed_mps?: number | null;
  climbing_time_seconds: number;
  climbing_elevation_gain_meters: number;
  climbing_vertical_rate_meters_per_hour?: number | null;
  average_aerobic_decoupling_percent?: number | null;
  z1_seconds: number;
  z2_zone_seconds: number;
  z3_seconds: number;
  z4_seconds: number;
  z5_seconds: number;
};

export type XcEventGoal = {
  event_name?: string | null;
  event_profile?: XcEventProfile | null;
  start_date: string;
  target_date: string;
  days_remaining: number;
  target_distance_meters: number;
  target_elevation_gain_meters: number;
  target_finish_time_seconds?: number | null;
  target_finish_speed_mps?: number | null;
  target_climb_density_meters_per_kilometer: number;
  training_window_days: number;
  counted_ride_count: number;
  counted_distance_meters: number;
  counted_elevation_gain_meters: number;
};

export type XcReadinessGate = {
  key: XcReadinessGateKey;
  label: string;
  status: XcReadinessStatus;
  unit: TrainingMetricUnit;
  direction: TrainingGoalDirection;
  current_value?: number | null;
  target_value?: number | null;
  gap_value?: number | null;
  progress_percent?: number | null;
  detail: string;
};

export type XcReadinessSummary = {
  status: XcReadinessStatus;
  title: string;
  reason: string;
  missing_most?: string | null;
  gates: XcReadinessGate[];
};

export type XcTrainingDeficit = {
  key: XcTrainingDeficitKey;
  priority: TrainingRecommendationPriority;
  title: string;
  detail: string;
  gap_value?: number | null;
  gap_unit?: TrainingMetricUnit | null;
  suggested_ride: XcSuggestedRide;
};

export type XcGoalProgress = {
  generated_at: string;
  event_goal?: XcEventGoal | null;
  readiness?: XcReadinessSummary | null;
  deficits: XcTrainingDeficit[];
  summary: XcProgressSummary;
  race_results: XcRaceResult[];
  goals: TrainingGoalMetric[];
  recommendations: TrainingRecommendation[];
  weekly_progress: XcWeeklyProgressPoint[];
  recent_rides: XcRideProgress[];
};

export type DhProgressSummary = {
  segment_count: number;
  session_count: number;
  effort_count: number;
  average_efforts_per_session?: number | null;
  average_repeat_fade_percent?: number | null;
  average_top_3_gap_percent?: number | null;
};

export type DhSegmentProgress = {
  segment_id: number;
  segment_title: string;
  effort_count: number;
  personal_record_duration_seconds?: number | null;
  recent_best_duration_seconds?: number | null;
  rolling_top_3_average_duration_seconds?: number | null;
  top_3_pr_gap_percent?: number | null;
  repeat_fade_percent?: number | null;
  latest_activity_id?: number | null;
  latest_activity_title?: string | null;
  latest_activity_started_at?: string | null;
};

export type DhSessionSummary = {
  activity_id: number;
  activity_title: string;
  started_at: string;
  segment_count: number;
  effort_count: number;
  fastest_effort_duration_seconds?: number | null;
  average_repeat_fade_percent?: number | null;
};

export type DhGoalProgress = {
  generated_at: string;
  summary: DhProgressSummary;
  goals: TrainingGoalMetric[];
  recommendations: TrainingRecommendation[];
  segments: DhSegmentProgress[];
  recent_sessions: DhSessionSummary[];
};

export type TrainingReportBoundary =
  | "day"
  | "week"
  | "month"
  | "3month"
  | "6month"
  | "1year"
  | "2year";

export type TrainingReportId =
  | "ride_summary"
  | "endurance"
  | "climbing"
  | "fatigue"
  | "compare_rides"
  | "aggregate_trends";

export type TrainingReportFilterKey =
  | "activity_ids"
  | "min_duration"
  | "min_distance";

export type TrainingReportMetricDirection = "higher" | "lower" | "neutral";

export type TrainingReportMetricDefinition = {
  key: string;
  label: string;
  unit?: string | null;
  direction: TrainingReportMetricDirection;
};

export type TrainingReportDefinition = {
  id: TrainingReportId;
  display_name: string;
  short_purpose: string;
  supported_filters: TrainingReportFilterKey[];
  required_data_quality: string[];
  result_sections: string[];
  metrics: TrainingReportMetricDefinition[];
};

export type TrainingReportDefinitionsResponse = {
  reports: TrainingReportDefinition[];
};

export type TrainingReportPoint = {
  bucket_start: string;
  bucket_end: string;
  z2_average_speed_mps?: number | null;
  average_aerobic_decoupling_percent?: number | null;
  climbing_pace_feet_per_week?: number | null;
  z1_seconds: number;
  z2_seconds: number;
  z3_seconds: number;
  z4_seconds: number;
  z5_seconds: number;
  elevation_gain_meters: number;
  elevation_gain_feet: number;
};

export type TrainingReportsResponse = {
  generated_at: string;
  boundary: TrainingReportBoundary;
  range_start: string;
  range_end: string;
  points: TrainingReportPoint[];
  ride_summary?: RideSummaryReport | null;
  endurance?: EnduranceReport | null;
  climbing?: ClimbingReport | null;
  fatigue?: FatigueReport | null;
  compare_rides?: CompareRidesReport | null;
};

export type RideSummaryReport = {
  activity_count: number;
  total_distance_meters: number;
  total_distance_miles: number;
  total_elevation_gain_meters: number;
  total_elevation_gain_feet: number;
  total_elapsed_seconds: number;
  total_moving_seconds: number;
  total_stopped_seconds: number;
  average_speed_mps?: number | null;
  average_speed_mph?: number | null;
  average_heart_rate_bpm?: number | null;
  max_heart_rate_bpm?: number | null;
  climbing_density_feet_per_hour?: number | null;
  z1_seconds: number;
  z2_seconds: number;
  z3_seconds: number;
  z4_seconds: number;
  z5_seconds: number;
  data_quality_flags: string[];
};

export type HourlyDurability = {
  hour: number;
  elapsed_start_seconds: number;
  elapsed_end_seconds: number;
  distance_meters?: number | null;
  average_speed_mps?: number | null;
  average_heart_rate_bpm?: number | null;
  max_heart_rate_bpm?: number | null;
  ascent_meters: number;
  moving_seconds: number;
  stopped_seconds: number;
  stop_count: number;
  efficiency_mps_per_bpm?: number | null;
};

export type EnduranceReport = {
  activity_count: number;
  median_aerobic_decoupling_percent?: number | null;
  median_late_speed_change_percent?: number | null;
  rides: EnduranceRide[];
};

export type EnduranceRide = {
  activity_id: number;
  title: string;
  started_at: string;
  elapsed_seconds: number;
  first_half_efficiency_mps_per_bpm?: number | null;
  second_half_efficiency_mps_per_bpm?: number | null;
  aerobic_decoupling_percent?: number | null;
  late_speed_change_percent?: number | null;
  late_heart_rate_change_percent?: number | null;
  hourly: HourlyDurability[];
};

export type FatigueReport = {
  activity_count: number;
  rides: FatigueRide[];
};

export type FatigueRide = {
  activity_id: number;
  title: string;
  started_at: string;
  elapsed_seconds: number;
  fatigue_start_hour?: number | null;
  hourly: HourlyDurability[];
};

export type ClimbingReport = {
  activity_count: number;
  climb_count: number;
  longest_climb?: ClimbReportRow | null;
  fastest_vertical_rate?: ClimbReportRow | null;
  median_climb?: ClimbReportRow | null;
  percentile_95_climb?: ClimbReportRow | null;
  first_half_median?: ClimbReportRow | null;
  second_half_median?: ClimbReportRow | null;
  best_climb?: ClimbReportRow | null;
  worst_climb?: ClimbReportRow | null;
  climbs: ClimbReportRow[];
};

export type ClimbReportRow = {
  activity_id: number;
  activity_title: string;
  climb_number: number;
  start_seconds: number;
  summit_seconds: number;
  duration_seconds: number;
  distance_meters: number;
  gain_meters: number;
  average_grade_percent?: number | null;
  vertical_rate_meters_per_hour: number;
  average_speed_mps?: number | null;
  average_heart_rate_bpm?: number | null;
  peak_heart_rate_bpm?: number | null;
  average_cadence_rpm?: number | null;
  average_power_watts?: number | null;
  heart_rate_recovery_30_seconds_bpm?: number | null;
  heart_rate_recovery_60_seconds_bpm?: number | null;
  seconds_to_drop_10_bpm?: number | null;
  seconds_to_drop_15_bpm?: number | null;
  summit_immediately_enters_descent: boolean;
  first_or_second_half: string;
};

export type CompareRidesReport = {
  candidates: CompareRideCandidate[];
  selected_rides: CompareRideColumn[];
  metrics: CompareRideMetric[];
};

export type CompareRideCandidate = {
  activity_id: number;
  title: string;
  started_at: string;
  distance_meters?: number | null;
  elevation_gain_meters?: number | null;
  moving_time_seconds?: number | null;
  total_time_seconds?: number | null;
};

export type CompareRideColumn = {
  activity_id: number;
  title: string;
  started_at: string;
  distance_meters?: number | null;
  elevation_gain_meters?: number | null;
  elapsed_seconds: number;
};

export type CompareRideMetric = {
  key: string;
  label: string;
  unit?: string | null;
  direction: string;
  trend?: CompareRideMetricTrend | null;
  values: CompareRideMetricValue[];
};

export type CompareRideMetricTrend = {
  first_activity_id: number;
  latest_activity_id: number;
  change?: number | null;
  change_percent?: number | null;
  display: string;
  interpretation: string;
};

export type CompareRideMetricValue = {
  activity_id: number;
  value?: number | null;
  display: string;
};

export type AdminAnalyticsBackfillResponse = {
  user_count: number;
  segment_count: number;
  fitness_task_count: number;
  segment_task_count: number;
  total_tasks_enqueued: number;
  segment_chunk_size: number;
};

export type ReprocessUserActivityImportsResponse = {
  user_id: number;
  status: string;
  message: string;
};

export type ReprocessActivityImportResponse = {
  activity_id: number;
  activity_import_id: number;
  user_id: number;
  status: string;
  message: string;
  task_id: string;
  task_status: string;
};

export type RegenerateUserSegmentsResponse = {
  user_id: number;
  status: string;
  message: string;
};

export type RegenerateSegmentEffortsResponse = {
  segment_id: number;
  status: string;
  message: string;
  task_id: string;
  task_status: string;
};

export type CleanupUserDuplicateActivitiesResponse = {
  user_id: number;
  status: string;
  message: string;
  duplicate_group_count: number;
  deleted_activity_count: number;
  retained_activity_count: number;
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
  processing_stage: string;
  processing_error?: string | null;
  size_bytes: number;
  mime_type?: string | null;
  created_at: string;
  activity_started_at?: string | null;
  activity_duration_seconds?: number | null;
  activity_location?: string | null;
};

export type NamedStat = {
  key: string;
  label: string;
  desc: string;
  value: number;
  error?: string | null;
};

export type SystemMetrics = Record<string, NamedStat[]>;

export function useAdminMetrics() {
  const response = $api.useQuery("get", "/admin/metrics", {});

  return {
    ...response,
    data: (response.data ?? null) as SystemMetrics | null,
  };
}

export function useAdminAppMetrics() {
  const response = $api.useQuery("get", "/admin/metrics/app", {});

  return {
    ...response,
    data: (response.data ?? []) as NamedStat[],
  };
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

export function useAdminBackfillUserXcTraining() {
  const mutation = $api.useMutation("post", "/admin/training/xc-backfill");

  return {
    ...mutation,
    backfillAsync: async (userId: number | string) => {
      const numericUserId = Number(userId);
      const result = await mutation.mutateAsync({
        body: {
          user_id: numericUserId,
        },
      });

      return result as ReprocessUserActivityImportsResponse;
    },
  };
}

export function useReprocessUserActivityImports() {
  const mutation = $api.useMutation(
    "post",
    "/admin/activity-imports/reprocess",
  );

  return {
    ...mutation,
    reprocessAsync: async (userId: number | string) => {
      const numericUserId = Number(userId);
      const result = await mutation.mutateAsync({
        body: {
          user_id: numericUserId,
        },
      });

      return result as ReprocessUserActivityImportsResponse;
    },
  };
}

export function useReprocessActivityImport() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation(
    "post",
    "/admin/activity-imports/reprocess-activity",
  );

  return {
    ...mutation,
    reprocessAsync: async (activityId: number | string) => {
      const numericActivityId = Number(activityId);
      const result = await mutation.mutateAsync({
        body: {
          activity_id: numericActivityId,
        },
      });

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["get", "/activities"] }),
        queryClient.invalidateQueries({ queryKey: ["get", "/activities/{id}"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/activity-imports"],
        }),
        queryClient.invalidateQueries({ queryKey: ["get", "/segments"] }),
        queryClient.invalidateQueries({ queryKey: ["get", "/segments/{id}"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/segments/{id}/comparison"],
        }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/training/xc-progress"],
        }),
      ]);

      return result as ReprocessActivityImportResponse;
    },
  };
}

export function useRegenerateUserSegments() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("post", "/admin/segments/regenerate");

  return {
    ...mutation,
    regenerateAsync: async (userId: number | string) => {
      const numericUserId = Number(userId);
      const result = await mutation.mutateAsync({
        body: {
          user_id: numericUserId,
        },
      });

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["get", "/segments"] }),
        queryClient.invalidateQueries({ queryKey: ["get", "/segments/{id}"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/segments/{id}/comparison"],
        }),
      ]);

      return result as RegenerateUserSegmentsResponse;
    },
  };
}

export function useRegenerateSegmentEfforts() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation(
    "post",
    "/admin/segments/regenerate-efforts",
  );

  return {
    ...mutation,
    regenerateAsync: async (segmentId: number | string) => {
      const numericSegmentId = Number(segmentId);
      const result = await mutation.mutateAsync({
        body: {
          segment_id: numericSegmentId,
        },
      });

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["get", "/segments"] }),
        queryClient.invalidateQueries({ queryKey: ["get", "/segments/{id}"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/segments/{id}/comparison"],
        }),
      ]);

      return result as RegenerateSegmentEffortsResponse;
    },
  };
}

export function useCleanupUserDuplicateActivities() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation(
    "post",
    "/admin/activity-imports/cleanup-duplicates",
  );

  return {
    ...mutation,
    cleanupAsync: async (userId: number | string) => {
      const numericUserId = Number(userId);
      const result = await mutation.mutateAsync({
        body: {
          user_id: numericUserId,
        },
      });

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["get", "/activities"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/activity-imports"],
        }),
        queryClient.invalidateQueries({ queryKey: ["get", "/segments"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/segments/{id}"],
        }),
        queryClient.invalidateQueries({ queryKey: ["get", "/fitness"] }),
      ]);

      return result as CleanupUserDuplicateActivitiesResponse;
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

  const pageData = response.data as PaginatedResponse<Activity> | undefined;

  return {
    ...response,
    data: pageData?.data,
    metadata: pageData?.metadata,
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
        queryClient.invalidateQueries({
          queryKey: ["get", "/segments/{id}/comparison"],
        }),
      ]);

      return result as Activity;
    },
  };
}

export function useUpdateActivity() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("patch", "/activities/{id}");

  return {
    ...mutation,
    updateAsync: async (id: number | string, body: UpdateActivityInput) => {
      const numericId = Number(id);
      const result = await mutation.mutateAsync({
        params: { path: { id: numericId } },
        body,
      });

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["get", "/activities"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/activities/{id}"],
        }),
        queryClient.invalidateQueries({ queryKey: ["get", "/training/xc-progress"] }),
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
        queryClient.invalidateQueries({
          queryKey: ["get", "/segments/{id}/comparison"],
        }),
        queryClient.invalidateQueries({ queryKey: ["get", "/fitness"] }),
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
    data: response.data as Segment[] | undefined,
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

export function useSegmentComparison(id: number | string | null | undefined) {
  const numericId = Number(id);
  const enabled = Number.isFinite(numericId) && numericId > 0;
  const response = $api.useQuery("get", "/segments/{id}/comparison", {
    params: { path: { id: enabled ? numericId : 0 } },
    options: { enabled },
  });

  return {
    ...response,
    data: (response.data ?? null) as SegmentComparison | null,
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
        queryClient.invalidateQueries({
          queryKey: ["get", "/segments/{id}/comparison"],
        }),
        queryClient.invalidateQueries({ queryKey: ["get", "/activities"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/activities/{id}"],
        }),
      ]);

      return result as Segment;
    },
  };
}

export function useCreateSegmentFromActivity() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("post", "/segments/from-activity");

  return {
    ...mutation,
    createAsync: async (payload: {
      activityId: number | string;
      title: string;
      startRoutePointIndex: number;
      endRoutePointIndex: number;
    }) => {
      const result = await mutation.mutateAsync({
        body: {
          activity_id: Number(payload.activityId),
          title: payload.title,
          start_route_point_index: payload.startRoutePointIndex,
          end_route_point_index: payload.endRoutePointIndex,
        },
      });

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["get", "/segments"] }),
        queryClient.invalidateQueries({ queryKey: ["get", "/segments/{id}"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/segments/{id}/comparison"],
        }),
        queryClient.invalidateQueries({ queryKey: ["get", "/activities"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/activities/{id}"],
        }),
      ]);

      return result as Segment;
    },
  };
}

export function useUpdateSegmentFromActivity() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("put", "/segments/{id}/from-activity");

  return {
    ...mutation,
    updateAsync: async (payload: {
      id: number | string;
      activityId: number | string;
      title: string;
      startRoutePointIndex: number;
      endRoutePointIndex: number;
    }) => {
      const numericId = Number(payload.id);
      const result = await mutation.mutateAsync({
        params: { path: { id: numericId } },
        body: {
          activity_id: Number(payload.activityId),
          title: payload.title,
          start_route_point_index: payload.startRoutePointIndex,
          end_route_point_index: payload.endRoutePointIndex,
        },
      });

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["get", "/segments"] }),
        queryClient.invalidateQueries({ queryKey: ["get", "/segments/{id}"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/segments/{id}/comparison"],
        }),
        queryClient.invalidateQueries({ queryKey: ["get", "/activities"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/activities/{id}"],
        }),
      ]);

      return result as Segment;
    },
  };
}

export function useUpdateSegment() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("put", "/segments/{id}");

  return {
    ...mutation,
    updateAsync: async (payload: {
      id: number | string;
      title?: string;
      mode?: SegmentMode;
      starred?: boolean;
    }) => {
      const numericId = Number(payload.id);
      const result = await mutation.mutateAsync({
        params: { path: { id: numericId } },
        body: {
          title: payload.title,
          mode: payload.mode,
          starred: payload.starred,
        },
      });

      const updatedSegment = result as Segment;

      queryClient.setQueriesData<Segment[] | undefined>(
        { queryKey: ["get", "/segments"] },
        (current) =>
          current?.map((segment) =>
            segment.id === numericId
              ? { ...segment, ...updatedSegment }
              : segment,
          ) ?? current,
      );
      queryClient.setQueriesData<Segment | null | undefined>(
        { queryKey: ["get", "/segments/{id}"] },
        (current) =>
          current && current.id === numericId
            ? { ...current, ...updatedSegment }
            : current,
      );

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["get", "/segments"] }),
        queryClient.invalidateQueries({ queryKey: ["get", "/segments/{id}"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/segments/{id}/comparison"],
        }),
        queryClient.invalidateQueries({ queryKey: ["get", "/activities"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/activities/{id}"],
        }),
      ]);

      return updatedSegment;
    },
  };
}

export function useDeleteSegment() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("delete", "/segments/{id}");

  return {
    ...mutation,
    deleteAsync: async (id: number | string) => {
      const numericId = Number(id);
      const result = await mutation.mutateAsync({
        params: { path: { id: numericId } },
      });

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["get", "/segments"] }),
        queryClient.invalidateQueries({ queryKey: ["get", "/segments/{id}"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/segments/{id}/comparison"],
        }),
        queryClient.invalidateQueries({ queryKey: ["get", "/activities"] }),
        queryClient.invalidateQueries({
          queryKey: ["get", "/activities/{id}"],
        }),
      ]);

      return result;
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
    data: response.data as ActivityImport[] | undefined,
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
    data: response.data as ActivityArchiveImportJob[] | undefined,
  };
}

export function useActivityProcessingState(opts?: {
  enabled?: boolean;
  refetchIntervalMs?: number | false;
}) {
  const response = $api.useQuery("get", "/activity-imports/processing-state", {
    options: {
      enabled: opts?.enabled ?? true,
      refetchInterval: opts?.refetchIntervalMs ?? false,
    },
  });

  return {
    ...response,
    data: (response.data ?? {
      is_active: false,
      source: null,
      source_label: null,
      stage: null,
      stage_label: null,
      message: null,
    }) as ActivityProcessingState,
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

      try {
        const result = await mutation.mutateAsync({ body: form });

        await Promise.all([
          queryClient.invalidateQueries({ queryKey: ["get", "/activities"] }),
          queryClient.invalidateQueries({
            queryKey: ["get", "/activity-imports"],
          }),
          queryClient.invalidateQueries({
            queryKey: ["get", "/activity-imports/processing-state"],
          }),
        ]);

        return result as ActivityImport;
      } catch (error) {
        await Promise.all([
          queryClient.invalidateQueries({
            queryKey: ["get", "/activity-imports"],
          }),
          queryClient.invalidateQueries({
            queryKey: ["get", "/activity-imports/processing-state"],
          }),
        ]);

        throw error;
      }
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

export function useStravaIntegrationEvents(opts?: {
  enabled?: boolean;
  refetchIntervalMs?: number | false;
}) {
  const response = $api.useQuery("get", "/integration-events/strava", {
    options: {
      enabled: opts?.enabled ?? true,
      refetchInterval: opts?.refetchIntervalMs ?? false,
    },
  });

  return {
    ...response,
    data: response.data as IntegrationEvent[] | undefined,
  };
}

export function useAdminIntegrationEvents(opts?: {
  enabled?: boolean;
  provider?: string;
  userId?: number | null;
  activityId?: number | null;
  importId?: number | null;
  limit?: number;
  refetchIntervalMs?: number | false;
}) {
  const response = $api.useQuery("get", "/admin/integration-events", {
    params: {
      query: {
        provider: opts?.provider,
        user_id: opts?.userId ?? undefined,
        activity_id: opts?.activityId ?? undefined,
        import_id: opts?.importId ?? undefined,
        limit: opts?.limit,
      },
    },
    options: {
      enabled: opts?.enabled ?? true,
      refetchInterval: opts?.refetchIntervalMs ?? false,
    },
  });

  return {
    ...response,
    data: response.data as IntegrationEvent[] | undefined,
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
        queryClient.invalidateQueries({
          queryKey: ["get", "/integration-events/strava"],
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
        queryClient.invalidateQueries({
          queryKey: ["get", "/integration-events/strava"],
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

export function useGarminIqLinkedDevices(opts?: {
  enabled?: boolean;
  refetchIntervalMs?: number | false;
}) {
  const response = $api.useQuery("get", "/garmin-iq/devices", {
    options: {
      enabled: opts?.enabled ?? true,
      refetchInterval: opts?.refetchIntervalMs ?? false,
    },
  });

  return {
    ...response,
    data: response.data as GarminIqLinkedDevice[] | undefined,
  };
}

export function useCompleteGarminIqLink() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("post", "/garmin-iq/link/complete");

  return {
    ...mutation,
    completeAsync: async (pairingCode: string) => {
      const result = await mutation.mutateAsync({
        body: {
          pairing_code: pairingCode,
        },
      });

      await queryClient.invalidateQueries({ queryKey: ["get", "/garmin-iq/devices"] });

      return result as GarminIqCompleteLinkResponse;
    },
  };
}

export function useUnlinkGarminIqDevice() {
  const queryClient = useQueryClient();
  const mutation = $api.useMutation("delete", "/garmin-iq/devices/{id}");

  return {
    ...mutation,
    unlinkAsync: async (id: number) => {
      const result = await mutation.mutateAsync({
        params: {
          path: {
            id,
          },
        },
      });

      await queryClient.invalidateQueries({ queryKey: ["get", "/garmin-iq/devices"] });

      return result as { message: string };
    },
  };
}

export function useUserPreferences(opts?: {
  enabled?: boolean;
  refetchIntervalMs?: number | false;
}) {
  const response = $api.useQuery("get", "/preferences", {
    options: {
      enabled: opts?.enabled ?? true,
      refetchInterval: opts?.refetchIntervalMs ?? false,
    },
  });

  return {
    ...response,
    data: (response.data ?? {
      unit_system: "mixed",
      estimated_ftp_watts: null,
      heart_rate_zone_bounds_bpm: null,
      xc_goal_event_name: null,
      xc_goal_start_date: null,
      xc_goal_target_date: null,
      xc_goal_target_distance_meters: null,
      xc_goal_target_elevation_gain_meters: null,
      xc_goal_target_finish_time_seconds: null,
      xc_goal_event_profile: null,
      xc_goal_backfill_status: null,
      xc_goal_backfill_completed_at: null,
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

export function useXcGoalProgress(opts?: { enabled?: boolean }) {
  const response = $api.useQuery("get", "/training/xc-progress", {
    options: { enabled: opts?.enabled ?? true },
  });

  return {
    ...response,
    data: (response.data ?? null) as XcGoalProgress | null,
  };
}

export function useDhGoalProgress(opts?: { enabled?: boolean }) {
  const response = $api.useQuery("get", "/training/dh-progress", {
    options: { enabled: opts?.enabled ?? true },
  });

  return {
    ...response,
    data: (response.data ?? null) as DhGoalProgress | null,
  };
}

export function useTrainingReports(
  boundary: TrainingReportBoundary,
  opts?: {
    enabled?: boolean;
    startDate?: string;
    endDate?: string;
    minDurationSeconds?: number;
    minDistanceMeters?: number;
  },
) {
  const response = $api.useQuery("get", "/training/reports", {
    params: {
      query: {
        boundary,
        start_date: opts?.startDate,
        end_date: opts?.endDate,
        min_duration_seconds: opts?.minDurationSeconds,
        min_distance_meters: opts?.minDistanceMeters,
      },
    },
    options: { enabled: opts?.enabled ?? true },
  });

  return {
    ...response,
    data: (response.data ?? null) as TrainingReportsResponse | null,
  };
}

export function useRideSummaryReport(opts: {
  report?: "ride_summary" | "endurance" | "climbing" | "fatigue" | "compare_rides";
  boundary: TrainingReportBoundary;
  startDate: string;
  endDate: string;
  activityIds?: number[];
  minDurationSeconds?: number;
  minDistanceMeters?: number;
  enabled?: boolean;
}) {
  const response = $api.useQuery("get", "/training/reports", {
    params: {
      query: {
        boundary: opts.boundary,
        report: opts.report ?? "ride_summary",
        start_date: opts.startDate,
        end_date: opts.endDate,
        min_duration_seconds: opts.minDurationSeconds,
        min_distance_meters: opts.minDistanceMeters,
        activity_ids:
          opts.activityIds && opts.activityIds.length > 0
            ? opts.activityIds.join(",")
            : undefined,
      },
    },
    options: { enabled: opts.enabled ?? true },
  });

  return {
    ...response,
    data: (response.data ?? null) as TrainingReportsResponse | null,
  };
}

export function useTrainingReportDefinitions() {
  const response = $api.useQuery("get", "/training/reports/definitions", {});

  return {
    ...response,
    data: (response.data ?? null) as TrainingReportDefinitionsResponse | null,
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
      await queryClient.invalidateQueries({
        queryKey: ["get", "/training/xc-progress"],
      });
      await queryClient.invalidateQueries({
        queryKey: ["get", "/activity-imports/processing-state"],
      });

      return result as UserPreferences;
    },
  };
}
