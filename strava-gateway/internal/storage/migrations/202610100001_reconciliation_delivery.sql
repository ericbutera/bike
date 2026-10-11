-- +goose Up
ALTER TABLE gateway_sync_jobs ADD COLUMN reconciliation boolean NOT NULL DEFAULT false;
ALTER TABLE strava_delivery_outbox DROP CONSTRAINT strava_delivery_outbox_status_check;
ALTER TABLE strava_delivery_outbox ADD CONSTRAINT strava_delivery_outbox_status_check
    CHECK (status IN ('waiting_for_fetch', 'pending', 'processing', 'delivered', 'dead', 'unchanged'));

-- +goose Down
DELETE FROM strava_delivery_outbox WHERE status='unchanged';
ALTER TABLE strava_delivery_outbox DROP CONSTRAINT strava_delivery_outbox_status_check;
ALTER TABLE strava_delivery_outbox ADD CONSTRAINT strava_delivery_outbox_status_check
    CHECK (status IN ('waiting_for_fetch', 'pending', 'processing', 'delivered', 'dead'));
ALTER TABLE gateway_sync_jobs DROP COLUMN reconciliation;
