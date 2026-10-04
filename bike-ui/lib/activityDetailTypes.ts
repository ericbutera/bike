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

export type ActivityChartPoint = {
  elapsed_seconds: number;
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

export type ActivityHeartRateZone = {
  zone: number;
  label: string;
  min_bpm?: number | null;
  max_bpm?: number | null;
  duration_seconds: number;
  share_percent: number;
};

export type ActivityLap = {
  lap_index: number;
  title: string;
  duration_seconds?: number | null;
  distance_meters?: number | null;
  average_speed_mps?: number | null;
  average_heart_rate_bpm?: number | null;
  max_heart_rate_bpm?: number | null;
};

export type Activity = {
  id: number;
  title: string;
  sport: string;
  source: string;
  started_at: string;
  activity_type?: string;
  original_filename?: string | null;
  format?: string | null;
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
  segment_efforts?: ActivitySegmentEffort[] | null;
};
