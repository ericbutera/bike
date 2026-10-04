-- +goose Up
CREATE TABLE gateway_oauth_states (
    state_hash char(64) PRIMARY KEY,
    target text NOT NULL CHECK (target IN ('rust', 'go', 'cs')),
    site_user_id bigint NOT NULL CHECK (site_user_id > 0),
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX gateway_oauth_states_expires_idx ON gateway_oauth_states (expires_at);

-- +goose Down
DROP TABLE gateway_oauth_states;
