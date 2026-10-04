-- +goose Up
ALTER TABLE strava_webhook_events
    ADD COLUMN traceparent text,
    ADD COLUMN tracestate text;

ALTER TABLE gateway_sync_jobs
    ADD COLUMN traceparent text,
    ADD COLUMN tracestate text;

-- +goose Down
ALTER TABLE gateway_sync_jobs
    DROP COLUMN tracestate,
    DROP COLUMN traceparent;

ALTER TABLE strava_webhook_events
    DROP COLUMN tracestate,
    DROP COLUMN traceparent;
