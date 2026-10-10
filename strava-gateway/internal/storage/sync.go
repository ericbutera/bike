package storage

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"time"

	"github.com/ericbutera/bike/strava-gateway/internal/webhook"
	"github.com/jackc/pgx/v5"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/propagation"
)

type Syncs struct{ DB JobDatabase }

func (syncs Syncs) QueueDepth(ctx context.Context) (int64, error) {
	var depth int64
	err := syncs.DB.QueryRow(ctx, `SELECT count(*) FROM gateway_sync_jobs WHERE target='rust' AND status IN ('queued','processing')`).Scan(&depth)
	return depth, err
}

type SyncJob struct {
	CreatedAt     time.Time
	ID            int64
	AthleteID     int64
	Target        string
	Mode          string
	Page          int
	AfterEpoch    int64
	Attempts      int
	WaitingReason string
	TraceParent   *string
	TraceState    *string
}

// QueueDueReconciliation revisits the rolling activity window for each live
// site link. The active-job constraint keeps concurrent workers from queuing
// duplicate work, while the latest job timestamp bounds provider usage.
func (syncs Syncs) QueueDueReconciliation(ctx context.Context, now time.Time) (int64, error) {
	command, err := syncs.DB.Exec(ctx, `INSERT INTO gateway_sync_jobs
		(athlete_id,target,mode,after_epoch)
		SELECT links.athlete_id,links.target,'incremental',$2
		FROM gateway_site_links AS links
		JOIN gateway_connections AS connections ON connections.athlete_id=links.athlete_id
		WHERE links.enabled AND links.target='rust' AND connections.revoked_at IS NULL
		  AND NOT EXISTS (
		    SELECT 1 FROM gateway_sync_jobs AS recent
		    WHERE recent.athlete_id=links.athlete_id AND recent.target=links.target
		      AND recent.created_at>$1
		  )
		ON CONFLICT (athlete_id,target) WHERE status IN ('queued','processing') DO NOTHING`,
		now.Add(-6*time.Hour), now.Add(-30*24*time.Hour).Unix())
	if err != nil {
		return 0, err
	}
	return command.RowsAffected(), nil
}

func (syncs Syncs) Queue(ctx context.Context, link SiteLink, mode string) error {
	if mode != "initial" && mode != "incremental" && mode != "full" {
		return errors.New("invalid Strava sync mode")
	}
	after := syncAfterEpoch(mode, time.Now())
	carrier := propagation.MapCarrier{}
	otel.GetTextMapPropagator().Inject(ctx, carrier)
	_, err := syncs.DB.Exec(ctx, `INSERT INTO gateway_sync_jobs
		(athlete_id,target,mode,after_epoch,traceparent,tracestate) VALUES ($1,$2,$3,$4,$5,$6)
		ON CONFLICT (athlete_id,target) WHERE status IN ('queued','processing') DO NOTHING`,
		link.AthleteID, link.Target, mode, after,
		nullableTrace(carrier.Get("traceparent")), nullableTrace(carrier.Get("tracestate")))
	return err
}

func syncAfterEpoch(mode string, now time.Time) int64 {
	if mode == "initial" || mode == "incremental" {
		return now.Add(-30 * 24 * time.Hour).Unix()
	}
	return 0
}

type SyncStatus struct {
	Status        string
	WaitingReason *string
	NextAttemptAt *time.Time
}

func (syncs Syncs) Status(ctx context.Context, link SiteLink) (SyncStatus, error) {
	var status SyncStatus
	var waitingReason string
	var nextAttemptAt time.Time
	err := syncs.DB.QueryRow(ctx, `SELECT status,coalesce(waiting_reason,''),next_attempt_at
		FROM gateway_sync_jobs WHERE athlete_id=$1 AND target=$2 ORDER BY id DESC LIMIT 1`,
		link.AthleteID, link.Target).Scan(&status.Status, &waitingReason, &nextAttemptAt)
	if errors.Is(err, pgx.ErrNoRows) {
		return SyncStatus{Status: "never"}, nil
	}
	if err == nil && waitingReason != "" && status.Status == "queued" {
		status.WaitingReason, status.NextAttemptAt = &waitingReason, &nextAttemptAt
	}
	return status, err
}

func (syncs Syncs) Claim(ctx context.Context) (claimed *SyncJob, err error) {
	tx, err := syncs.DB.Begin(ctx)
	if err != nil {
		return nil, err
	}
	defer rollbackTransaction(ctx, tx, &err)
	var job SyncJob
	err = tx.QueryRow(ctx, `WITH candidate AS (
		SELECT id FROM gateway_sync_jobs
		WHERE target='rust' AND ((status='queued' AND next_attempt_at<=now())
		   OR (status='processing' AND lease_until<now()))
		ORDER BY next_attempt_at,id FOR UPDATE SKIP LOCKED LIMIT 1
	) UPDATE gateway_sync_jobs AS jobs
	SET status='processing', lease_until=now()+interval '2 minutes',
	    attempts=jobs.attempts+1, updated_at=now()
	FROM candidate WHERE jobs.id=candidate.id
	RETURNING jobs.id,jobs.athlete_id,jobs.target,jobs.mode,jobs.page,
	    coalesce(jobs.after_epoch,0),jobs.attempts,coalesce(jobs.waiting_reason,''),jobs.traceparent,jobs.tracestate,jobs.created_at`).Scan(
		&job.ID, &job.AthleteID, &job.Target, &job.Mode, &job.Page,
		&job.AfterEpoch, &job.Attempts, &job.WaitingReason, &job.TraceParent, &job.TraceState, &job.CreatedAt)
	if errors.Is(err, pgx.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if job.WaitingReason != "" {
		if _, err := tx.Exec(ctx, `INSERT INTO gateway_sync_events
			(sync_job_id,athlete_id,target,attempt,event_type,waiting_reason)
			VALUES ($1,$2,$3,$4,'resumed',$5)`, job.ID, job.AthleteID, job.Target, job.Attempts, job.WaitingReason); err != nil {
			return nil, err
		}
		if _, err := tx.Exec(ctx, `UPDATE gateway_sync_jobs SET waiting_reason=NULL
			WHERE id=$1 AND status='processing'`, job.ID); err != nil {
			return nil, err
		}
	}
	if err := tx.Commit(ctx); err != nil {
		return nil, err
	}
	return &job, nil
}

func (syncs Syncs) CompletePage(ctx context.Context, job SyncJob, ids []int64) (err error) {
	carrier := propagation.MapCarrier{}
	otel.GetTextMapPropagator().Inject(ctx, carrier)
	tx, err := syncs.DB.Begin(ctx)
	if err != nil {
		return err
	}
	defer rollbackTransaction(ctx, tx, &err)
	for _, id := range ids {
		event := webhook.Event{
			AspectType: "update", EventTime: time.Now().Unix(), ObjectID: id,
			ObjectType: "activity", OwnerID: job.AthleteID,
		}
		payload, err := json.Marshal(event)
		if err != nil {
			return err
		}
		key := sha256.Sum256([]byte(fmt.Sprintf("sync:%d:%d", job.ID, id)))
		var eventID int64
		if err := tx.QueryRow(ctx, `INSERT INTO strava_webhook_events
			(event_key,subscription_id,owner_id,object_id,object_type,aspect_type,event_time,payload,traceparent,tracestate,received_at)
			VALUES ($1,0,$2,$3,'activity','update',$4,$5,$6,$7,coalesce($8,now()))
			ON CONFLICT (event_key) DO UPDATE SET event_key=excluded.event_key
			RETURNING id`, hex.EncodeToString(key[:]), job.AthleteID, id,
			event.EventTime, payload, nullableTrace(carrier.Get("traceparent")), nullableTrace(carrier.Get("tracestate")), syncReceipt(job)).Scan(&eventID); err != nil {
			return err
		}
		if _, err := tx.Exec(ctx, `INSERT INTO strava_delivery_outbox
			(event_id,target,status) VALUES ($1,$2,'waiting_for_fetch')
			ON CONFLICT (event_id,target) DO NOTHING`, eventID, job.Target); err != nil {
			return err
		}
	}
	status := "queued"
	if len(ids) < 100 {
		status = "succeeded"
	}
	command, err := tx.Exec(ctx, `UPDATE gateway_sync_jobs SET status=$2,page=page+1,
		lease_until=NULL,next_attempt_at=now(),waiting_reason=NULL,updated_at=now()
		WHERE id=$1 AND status='processing'`, job.ID, status)
	if err != nil {
		return err
	}
	if command.RowsAffected() != 1 {
		return errors.New("strava sync lease was lost")
	}
	return tx.Commit(ctx)
}

func (syncs Syncs) Retry(ctx context.Context, job SyncJob, retryAt time.Time, failure, waitingReason string) (err error) {
	status := "queued"
	if job.Attempts >= 20 {
		status = "dead"
	}
	tx, err := syncs.DB.Begin(ctx)
	if err != nil {
		return err
	}
	defer rollbackTransaction(ctx, tx, &err)
	command, err := tx.Exec(ctx, `UPDATE gateway_sync_jobs SET status=$2,
		next_attempt_at=$3,lease_until=NULL,last_error=$4,waiting_reason=NULLIF($5,''),updated_at=now()
		WHERE id=$1 AND status='processing'`, job.ID, status, retryAt, truncateFailure(failure), waitingReason)
	if err != nil {
		return err
	}
	if command.RowsAffected() != 1 {
		return errors.New("strava sync lease was lost")
	}
	if waitingReason != "" && status == "queued" {
		if _, err := tx.Exec(ctx, `INSERT INTO gateway_sync_events
			(sync_job_id,athlete_id,target,attempt,event_type,waiting_reason)
			VALUES ($1,$2,$3,$4,'paused',$5)`, job.ID, job.AthleteID, job.Target, job.Attempts, waitingReason); err != nil {
			return err
		}
	}
	return tx.Commit(ctx)
}

func (syncs Syncs) Fail(ctx context.Context, job SyncJob, failure string) error {
	_, err := syncs.DB.Exec(ctx, `UPDATE gateway_sync_jobs SET status='dead',
		lease_until=NULL,last_error=$2,waiting_reason=NULL,updated_at=now()
		WHERE id=$1 AND status='processing'`, job.ID, truncateFailure(failure))
	return err
}

func syncReceipt(job SyncJob) *time.Time {
	if job.CreatedAt.IsZero() {
		return nil
	}
	return &job.CreatedAt
}
