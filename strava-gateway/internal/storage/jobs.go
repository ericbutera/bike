package storage

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"time"

	"github.com/ericbutera/bike/strava-gateway/internal/webhook"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"
)

type Jobs struct{ DB *pgxpool.Pool }

type QueueStat struct {
	Depth     int64
	OldestAge time.Duration
}

type QueueStats struct {
	Webhooks   QueueStat
	Deliveries QueueStat
	Syncs      QueueStat
	Stuck      [3]int64
	Dead       [3]int64
}

func (jobs Jobs) QueueStats(ctx context.Context) (QueueStats, error) {
	var stats QueueStats
	var eventAgeSeconds, deliveryAgeSeconds, syncAgeSeconds float64
	if err := jobs.DB.QueryRow(ctx, `SELECT
		count(*) FILTER (WHERE status IN ('pending','processing')),
		coalesce(extract(epoch FROM now()-(min(received_at) FILTER (WHERE status='pending'))),0),
		(SELECT count(*) FROM strava_delivery_outbox WHERE target='rust' AND status IN ('pending','processing')),
		coalesce((SELECT extract(epoch FROM now()-min(created_at)) FROM strava_delivery_outbox WHERE target='rust' AND status='pending'),0),
		(SELECT count(*) FROM gateway_sync_jobs WHERE target='rust' AND status IN ('queued','processing')),
		coalesce((SELECT extract(epoch FROM now()-min(created_at)) FROM gateway_sync_jobs WHERE target='rust' AND status='queued' AND (next_attempt_at<=now() OR waiting_reason IS NULL)),0),
		(SELECT count(*) FROM strava_webhook_events WHERE status='processing' AND lease_until<now()),
		(SELECT count(*) FROM strava_delivery_outbox WHERE target='rust' AND status='processing' AND lease_until<now()),
		(SELECT count(*) FROM gateway_sync_jobs WHERE target='rust' AND status='processing' AND lease_until<now()),
		(SELECT count(*) FROM strava_webhook_events WHERE status='dead'),
		(SELECT count(*) FROM strava_delivery_outbox WHERE target='rust' AND status='dead'),
		(SELECT count(*) FROM gateway_sync_jobs WHERE target='rust' AND status='dead')
		FROM strava_webhook_events`).Scan(&stats.Webhooks.Depth, &eventAgeSeconds,
		&stats.Deliveries.Depth, &deliveryAgeSeconds, &stats.Syncs.Depth, &syncAgeSeconds,
		&stats.Stuck[0], &stats.Stuck[1], &stats.Stuck[2],
		&stats.Dead[0], &stats.Dead[1], &stats.Dead[2]); err != nil {
		return QueueStats{}, err
	}
	stats.Webhooks.OldestAge = time.Duration(math.Max(0, eventAgeSeconds) * float64(time.Second))
	stats.Deliveries.OldestAge = time.Duration(math.Max(0, deliveryAgeSeconds) * float64(time.Second))
	stats.Syncs.OldestAge = time.Duration(math.Max(0, syncAgeSeconds) * float64(time.Second))
	return stats, nil
}

type EventJob struct {
	ID          int64
	Event       webhook.Event
	Attempts    int
	TraceParent *string
	TraceState  *string
}

func (jobs Jobs) ClaimEvent(ctx context.Context) (*EventJob, error) {
	var job EventJob
	var raw []byte
	err := jobs.DB.QueryRow(ctx, `WITH candidate AS (
		SELECT id FROM strava_webhook_events
		WHERE (status='pending' AND next_attempt_at<=now())
		   OR (status='processing' AND lease_until<now())
		ORDER BY next_attempt_at, id FOR UPDATE SKIP LOCKED LIMIT 1
	) UPDATE strava_webhook_events AS events
	SET status='processing', lease_until=now()+interval '2 minutes',
	    attempts=attempts+1, updated_at=now()
	FROM candidate WHERE events.id=candidate.id
	RETURNING events.id, events.payload, events.attempts, events.traceparent, events.tracestate`).Scan(
		&job.ID, &raw, &job.Attempts, &job.TraceParent, &job.TraceState)
	if errors.Is(err, pgx.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if err := json.Unmarshal(raw, &job.Event); err != nil {
		return nil, fmt.Errorf("decode stored webhook event %d: %w", job.ID, err)
	}
	return &job, nil
}

type Artifact struct {
	SHA256       string
	RelativePath string
	SizeBytes    int64
}

func (jobs Jobs) CompleteEvent(ctx context.Context, job EventJob, operation string, artifact *Artifact) (err error) {
	if operation != "upsert" && operation != "delete" && operation != "deauthorize" {
		return errors.New("invalid delivery operation")
	}
	tx, err := jobs.DB.Begin(ctx)
	if err != nil {
		return err
	}
	defer rollbackTransaction(ctx, tx, &err)
	var hash *string
	if artifact != nil {
		if operation != "upsert" || artifact.SHA256 == "" || artifact.RelativePath == "" || artifact.SizeBytes <= 0 {
			return errors.New("invalid fetched artifact")
		}
		if _, err := tx.Exec(ctx, `INSERT INTO gateway_artifacts (event_id,sha256,relative_path,size_bytes)
			VALUES ($1,$2,$3,$4) ON CONFLICT (event_id) DO UPDATE SET
			sha256=excluded.sha256, relative_path=excluded.relative_path,
			size_bytes=excluded.size_bytes`,
			job.ID, artifact.SHA256, artifact.RelativePath, artifact.SizeBytes); err != nil {
			return err
		}
		hash = &artifact.SHA256
	} else if operation == "upsert" {
		return errors.New("upsert requires an artifact")
	}
	if _, err := tx.Exec(ctx, `UPDATE strava_delivery_outbox SET status='pending',
		operation=$2, artifact_sha256=$3, next_attempt_at=now(), updated_at=now()
		WHERE event_id=$1 AND target='rust' AND status IN ('waiting_for_fetch','pending')`, job.ID, operation, hash); err != nil {
		return err
	}
	command, err := tx.Exec(ctx, `UPDATE strava_webhook_events SET status='processed',
		lease_until=NULL, last_error=NULL, updated_at=now() WHERE id=$1 AND status='processing'`, job.ID)
	if err != nil {
		return err
	}
	if command.RowsAffected() != 1 {
		return errors.New("event lease was lost")
	}
	return tx.Commit(ctx)
}

func (jobs Jobs) RetryEvent(ctx context.Context, job EventJob, next time.Time, failure string, permanent bool) error {
	status := "pending"
	if permanent || job.Attempts >= 20 {
		status = "dead"
	}
	_, err := jobs.DB.Exec(ctx, `UPDATE strava_webhook_events SET status=$2,
		next_attempt_at=$3, lease_until=NULL, last_error=$4, updated_at=now()
		WHERE id=$1 AND status='processing'`, job.ID, status, next, truncateFailure(failure))
	return err
}

func truncateFailure(failure string) string {
	characters := []rune(failure)
	return string(characters[:min(len(characters), 500)])
}

func (jobs Jobs) IgnoreEvent(ctx context.Context, job EventJob) (err error) {
	tx, err := jobs.DB.Begin(ctx)
	if err != nil {
		return err
	}
	defer rollbackTransaction(ctx, tx, &err)
	if _, err := tx.Exec(ctx, `UPDATE strava_delivery_outbox SET status='delivered',
		updated_at=now() WHERE event_id=$1 AND target='rust' AND status='waiting_for_fetch'`, job.ID); err != nil {
		return err
	}
	if _, err := tx.Exec(ctx, `UPDATE strava_webhook_events SET status='processed',
		lease_until=NULL, updated_at=now() WHERE id=$1 AND status='processing'`, job.ID); err != nil {
		return err
	}
	return tx.Commit(ctx)
}

type DeliveryJob struct {
	ID          int64
	EventID     int64
	CreatedAt   time.Time
	Event       webhook.Event
	Target      string
	Operation   string
	Artifact    *Artifact
	Attempts    int
	TraceParent *string
	TraceState  *string
}

func (jobs Jobs) ClaimDelivery(ctx context.Context) (*DeliveryJob, error) {
	var job DeliveryJob
	var raw []byte
	var hash, path *string
	var size *int64
	err := jobs.DB.QueryRow(ctx, `WITH candidate AS (
		SELECT id FROM strava_delivery_outbox
		WHERE target='rust' AND ((status='pending' AND next_attempt_at<=now())
		   OR (status='processing' AND lease_until<now()))
		ORDER BY next_attempt_at, id FOR UPDATE SKIP LOCKED LIMIT 1
	) UPDATE strava_delivery_outbox AS outbox
	SET status='processing', lease_until=now()+interval '2 minutes',
	    attempts=outbox.attempts+1, updated_at=now()
	FROM candidate, strava_webhook_events AS events
	LEFT JOIN gateway_artifacts AS artifacts ON artifacts.event_id=events.id
	WHERE outbox.id=candidate.id AND events.id=outbox.event_id
		RETURNING outbox.id, outbox.event_id, events.payload, outbox.target,
	    outbox.operation, artifacts.sha256, artifacts.relative_path,
	    artifacts.size_bytes, outbox.created_at, outbox.attempts, events.traceparent, events.tracestate`).Scan(&job.ID, &job.EventID, &raw,
		&job.Target, &job.Operation, &hash, &path, &size, &job.CreatedAt, &job.Attempts, &job.TraceParent, &job.TraceState)
	if errors.Is(err, pgx.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if err := json.Unmarshal(raw, &job.Event); err != nil {
		return nil, fmt.Errorf("decode stored event for delivery %d: %w", job.ID, err)
	}
	if hash != nil && path != nil && size != nil {
		job.Artifact = &Artifact{SHA256: *hash, RelativePath: *path, SizeBytes: *size}
	}
	return &job, nil
}

func (jobs Jobs) CompleteDelivery(ctx context.Context, job DeliveryJob) error {
	command, err := jobs.DB.Exec(ctx, `UPDATE strava_delivery_outbox SET status='delivered',
		lease_until=NULL, last_error=NULL, updated_at=now()
		WHERE id=$1 AND status='processing'`, job.ID)
	if err != nil {
		return err
	}
	if command.RowsAffected() != 1 {
		return errors.New("delivery lease was lost")
	}
	return nil
}

func (jobs Jobs) RetryDelivery(ctx context.Context, job DeliveryJob, next time.Time, failure string, permanent bool) error {
	status := "pending"
	if permanent || job.Attempts >= 20 {
		status = "dead"
	}
	_, err := jobs.DB.Exec(ctx, `UPDATE strava_delivery_outbox SET status=$2,
		next_attempt_at=$3, lease_until=NULL, last_error=$4, updated_at=now()
		WHERE id=$1 AND status='processing'`, job.ID, status, next, truncateFailure(failure))
	return err
}

// RequeueMissingArtifact returns an already fetched event to the inbox so the
// current provider payload can be fetched again. Completed site deliveries
// remain completed; only undelivered targets wait for the new artifact.
func (jobs Jobs) RequeueMissingArtifact(ctx context.Context, job DeliveryJob) (err error) {
	if job.Operation != "upsert" || job.EventID <= 0 {
		return errors.New("only fetched activity deliveries can be refetched")
	}
	tx, err := jobs.DB.Begin(ctx)
	if err != nil {
		return err
	}
	defer rollbackTransaction(ctx, tx, &err)
	if _, err := tx.Exec(ctx, `UPDATE strava_webhook_events
		SET status='pending',next_attempt_at=now(),lease_until=NULL,attempts=0,updated_at=now()
		WHERE id=$1 AND status='processed'`, job.EventID); err != nil {
		return err
	}
	command, err := tx.Exec(ctx, `UPDATE strava_delivery_outbox
		SET status='waiting_for_fetch',lease_until=NULL,updated_at=now()
		WHERE event_id=$1 AND target='rust' AND status IN ('pending','processing')`, job.EventID)
	if err != nil {
		return err
	}
	if command.RowsAffected() == 0 {
		return errors.New("no undelivered targets remain for refetch")
	}
	return tx.Commit(ctx)
}
