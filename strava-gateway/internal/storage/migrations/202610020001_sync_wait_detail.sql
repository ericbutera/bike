-- +goose Up
ALTER TABLE gateway_sync_jobs ADD COLUMN waiting_reason text;

-- +goose Down
ALTER TABLE gateway_sync_jobs DROP COLUMN waiting_reason;
