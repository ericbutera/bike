-- +goose Up
CREATE TABLE gateway_connections (
    athlete_id bigint PRIMARY KEY,
    token_ciphertext bytea NOT NULL,
    token_key_version integer NOT NULL DEFAULT 1,
    expires_at timestamptz NOT NULL,
    scopes text[] NOT NULL,
    revoked_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE gateway_site_links (
    athlete_id bigint NOT NULL REFERENCES gateway_connections(athlete_id) ON DELETE CASCADE,
    target text NOT NULL CHECK (target IN ('rust', 'go', 'cs')),
    site_user_id bigint NOT NULL,
    enabled boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (athlete_id, target),
    UNIQUE (target, site_user_id)
);

CREATE TABLE gateway_artifacts (
    event_id bigint PRIMARY KEY REFERENCES strava_webhook_events(id) ON DELETE CASCADE,
    sha256 char(64) NOT NULL,
    relative_path text NOT NULL,
    size_bytes bigint NOT NULL CHECK (size_bytes > 0),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE gateway_rate_limits (
    bucket text PRIMARY KEY,
    limit_count integer NOT NULL CHECK (limit_count > 0),
    used_count integer NOT NULL DEFAULT 0 CHECK (used_count >= 0),
    reset_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE strava_delivery_outbox ADD COLUMN operation text NOT NULL DEFAULT 'upsert'
    CHECK (operation IN ('upsert', 'delete', 'deauthorize'));
ALTER TABLE strava_delivery_outbox DROP CONSTRAINT strava_delivery_outbox_status_check;
ALTER TABLE strava_delivery_outbox ADD CONSTRAINT strava_delivery_outbox_status_check
    CHECK (status IN ('waiting_for_fetch', 'pending', 'processing', 'delivered', 'dead'));
UPDATE strava_delivery_outbox SET operation = 'delete' WHERE event_id IN
    (SELECT id FROM strava_webhook_events WHERE aspect_type = 'delete' AND object_type = 'activity');
UPDATE strava_delivery_outbox SET operation = 'deauthorize' WHERE event_id IN
    (SELECT id FROM strava_webhook_events WHERE object_type = 'athlete');

-- +goose Down
ALTER TABLE strava_delivery_outbox DROP CONSTRAINT strava_delivery_outbox_status_check;
ALTER TABLE strava_delivery_outbox ADD CONSTRAINT strava_delivery_outbox_status_check
    CHECK (status IN ('waiting_for_fetch', 'pending', 'delivered', 'dead'));
ALTER TABLE strava_delivery_outbox DROP COLUMN operation;
DROP TABLE gateway_rate_limits;
DROP TABLE gateway_artifacts;
DROP TABLE gateway_site_links;
DROP TABLE gateway_connections;
