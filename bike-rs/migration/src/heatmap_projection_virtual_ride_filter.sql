DELETE FROM heatmap_chunks c
USING heatmap_projections p
WHERE c.activity_id = p.activity_id AND p.projection_version < 3;

UPDATE heatmap_user_states
SET revision = revision + 1
WHERE user_id IN (
    SELECT DISTINCT user_id FROM heatmap_projections WHERE projection_version < 3
);

UPDATE heatmap_projections
SET generation = generation + 1,
    projection_version = 3,
    status = 'pending',
    queued_at = NULL,
    error = NULL,
    min_x = NULL,
    min_y = NULL,
    max_x = NULL,
    max_y = NULL
WHERE projection_version < 3;

ALTER TABLE heatmap_projections
    ALTER COLUMN projection_version SET DEFAULT 3;

CREATE OR REPLACE FUNCTION heatmap_activity_changed() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND NEW.user_id = OLD.user_id
       AND NEW.sport = OLD.sport AND NEW.source = OLD.source
       AND NEW.title = OLD.title AND NEW.started_at = OLD.started_at
       AND NEW.derived_data_json::text IS NOT DISTINCT FROM OLD.derived_data_json::text THEN
        RETURN NEW;
    END IF;
    IF TG_OP <> 'INSERT' THEN
        INSERT INTO heatmap_user_states(user_id, revision)
        SELECT OLD.user_id, 1 WHERE EXISTS (SELECT 1 FROM users WHERE id=OLD.user_id)
        ON CONFLICT(user_id) DO UPDATE SET revision = heatmap_user_states.revision + 1;
    END IF;
    IF TG_OP <> 'DELETE' THEN
        INSERT INTO heatmap_user_states(user_id, revision) VALUES (NEW.user_id, 1)
        ON CONFLICT(user_id) DO UPDATE SET revision = heatmap_user_states.revision + 1;
        INSERT INTO heatmap_projections(activity_id, user_id) VALUES (NEW.id, NEW.user_id)
        ON CONFLICT(activity_id) DO UPDATE SET
            user_id = EXCLUDED.user_id, generation = heatmap_projections.generation + 1,
            status = 'pending', queued_at = NULL, error = NULL,
            min_x = NULL, min_y = NULL, max_x = NULL, max_y = NULL;
        RETURN NEW;
    END IF;
    RETURN OLD;
END;
$$;
