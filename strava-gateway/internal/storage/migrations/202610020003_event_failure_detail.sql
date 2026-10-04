-- +goose Up
ALTER TABLE strava_webhook_events ADD COLUMN last_error text;

-- +goose Down
ALTER TABLE strava_webhook_events DROP COLUMN last_error;
