SET TIME ZONE 'UTC';

DO $$
BEGIN
    IF (SELECT count(*) FROM users) <> 3
       OR (SELECT count(*) FROM user_preferences) <> 1
       OR (SELECT count(*) FROM activities) <> 2
       OR (SELECT count(*) FROM activity_imports) <> 2
       OR (SELECT count(*) FROM activity_import_artifacts) <> 2
       OR (SELECT count(*) FROM segments) <> 1
       OR (SELECT count(*) FROM segment_efforts) <> 2
       OR (SELECT count(*) FROM segment_summaries) <> 1
       OR (SELECT count(*) FROM segment_user_summaries) <> 1
       OR (SELECT count(*) FROM refresh_tokens) <> 0
       OR (SELECT count(*) FROM background_tasks) <> 5
       OR (SELECT count(*) FROM integration_events) <> 0
       OR (SELECT count(*) FROM activity_import_locks) <> 0
       OR (SELECT array_agg(id ORDER BY id) FROM activities) <> ARRAY[1109, 1110]
       OR (SELECT array_agg(id ORDER BY id) FROM segment_efforts) <> ARRAY[5895, 5912]
       OR (SELECT array_agg(id ORDER BY id) FROM segments) <> ARRAY[5]
       OR (SELECT array_agg(id ORDER BY id) FROM users) <> ARRAY[1, 2, 3]
       OR (SELECT disabled FROM users WHERE id = 2) <> false
       OR (SELECT disabled FROM users WHERE id = 3) <> true
       OR (SELECT array_agg(id ORDER BY id) FROM background_tasks) <> ARRAY[1, 2, 3, 4, 5]
       OR (SELECT jsonb_array_length(derived_data_json::jsonb->'route_points') FROM activities WHERE id = 1109) <> 11
       OR EXISTS (
            SELECT 1
            FROM activities
            CROSS JOIN LATERAL jsonb_array_elements(derived_data_json::jsonb->'route_points') AS point
            WHERE id IN (1109, 1110)
              AND (point->>'speed_mps')::numeric <> round((point->>'speed_mps')::numeric, 3)
       )
       OR EXISTS (
            SELECT 1
            FROM activities
            CROSS JOIN LATERAL jsonb_array_elements(derived_data_json::jsonb->'chart_points') AS point
            WHERE id IN (1109, 1110)
              AND (point->>'speed_mps')::numeric <> round((point->>'speed_mps')::numeric, 3)
       )
       OR (SELECT jsonb_array_length(route_data_json::jsonb) FROM segments WHERE id = 5) <> 11 THEN
        RAISE EXCEPTION 'four-views fixture counts or selected IDs differ';
    END IF;
END
$$;

SELECT md5(jsonb_build_object(
    'users', (SELECT jsonb_agg(jsonb_build_array(
        id, pid, email, name, is_admin, disabled, email_verified_at, created_at, updated_at)
        ORDER BY id) FROM users),
    'preferences', (SELECT jsonb_agg(jsonb_build_array(id, user_id, unit_system) ORDER BY id) FROM user_preferences),
    'activities', (SELECT jsonb_agg(jsonb_build_array(
        id, user_id, activity_import_id, title, sport, source, activity_type,
        started_at, ended_at, distance_meters, moving_time_seconds,
        derived_data_json::jsonb) ORDER BY id) FROM activities),
    'imports', (SELECT jsonb_agg(jsonb_build_array(
        id, user_id, activity_id, storage_path, size_bytes, status) ORDER BY id) FROM activity_imports),
    'artifacts', (SELECT jsonb_agg(jsonb_build_array(
        id, activity_import_id, storage_path, size_bytes, checksum_sha256) ORDER BY id) FROM activity_import_artifacts),
    'segments', (SELECT jsonb_agg(jsonb_build_array(
        id, user_id, title, mode, distance_meters, route_data_json::jsonb) ORDER BY id) FROM segments),
    'efforts', (SELECT jsonb_agg(jsonb_build_array(
        id, user_id, segment_id, activity_id, effort_index,
        start_route_point_index, end_route_point_index, duration_seconds,
        overall_rank, user_rank) ORDER BY id) FROM segment_efforts),
    'summaries', (SELECT jsonb_agg(jsonb_build_array(
        segment_id, effort_count, leader_effort_id, best_duration_seconds) ORDER BY segment_id)
        FROM segment_summaries),
    'tasks', (SELECT jsonb_agg(jsonb_build_array(
        id, task_type, payload::jsonb, status, attempts, max_attempts, error,
        result, scheduled_for, started_at, completed_at, created_at, updated_at)
        ORDER BY id) FROM background_tasks),
    'user_summaries', (SELECT jsonb_agg(jsonb_build_array(
        segment_id, user_id, effort_count, personal_best_effort_id,
        personal_best_duration_seconds) ORDER BY id) FROM segment_user_summaries)
)::text);
