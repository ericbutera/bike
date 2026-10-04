CREATE TABLE heatmap_user_states (
    user_id integer PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    revision bigint NOT NULL DEFAULT 1
);

CREATE TABLE heatmap_projections (
    activity_id integer PRIMARY KEY REFERENCES activities(id) ON DELETE CASCADE,
    user_id integer NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    generation bigint NOT NULL DEFAULT 1,
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','ready','skipped','failed')),
    projection_version integer NOT NULL DEFAULT 1,
    queued_at timestamptz,
    error text,
    min_x double precision, min_y double precision,
    max_x double precision, max_y double precision
);
CREATE INDEX heatmap_pending_idx ON heatmap_projections(activity_id) WHERE status='pending';
CREATE INDEX heatmap_progress_idx ON heatmap_projections(user_id, status);

CREATE TABLE heatmap_chunks (
    activity_id integer NOT NULL REFERENCES heatmap_projections(activity_id) ON DELETE CASCADE,
    band integer NOT NULL CHECK (band BETWEEN 0 AND 3),
    chunk_index integer NOT NULL,
    min_x double precision NOT NULL, min_y double precision NOT NULL,
    max_x double precision NOT NULL, max_y double precision NOT NULL,
    points bytea NOT NULL CHECK (octet_length(points) BETWEEN 32 AND 4096),
    PRIMARY KEY(activity_id, band, chunk_index)
);
-- Native PostgreSQL box/GiST indexing requires no extensions. Coordinates are
-- normalized Web Mercator; query boxes include the raster's seam gutter.
CREATE INDEX heatmap_chunk_bounds_idx ON heatmap_chunks USING gist
    (box(point(min_x, min_y), point(max_x, max_y)));

CREATE FUNCTION heatmap_activity_changed() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND NEW.user_id = OLD.user_id
       AND NEW.sport = OLD.sport AND NEW.source = OLD.source
       AND NEW.started_at = OLD.started_at
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
CREATE TRIGGER heatmap_activity_changed AFTER INSERT OR UPDATE OR DELETE ON activities
    FOR EACH ROW EXECUTE FUNCTION heatmap_activity_changed();

-- Historical backfill queues scalar identities only, never route JSON.
INSERT INTO heatmap_user_states(user_id) SELECT DISTINCT user_id FROM activities;
INSERT INTO heatmap_projections(activity_id, user_id) SELECT id, user_id FROM activities;
