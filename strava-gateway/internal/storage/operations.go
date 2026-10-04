package storage

import (
	"context"
	"errors"
	"fmt"
	"time"

	"github.com/jackc/pgx/v5/pgxpool"
)

var ErrDeadJobNotFound = errors.New("dead job not found")

type JobCount struct {
	Kind   string
	Status string
	Count  int64
}

type Operations struct{ DB *pgxpool.Pool }

type DeadJob struct {
	Kind         string    `json:"kind"`
	ID           int64     `json:"id"`
	Target       string    `json:"target"`
	AthleteID    int64     `json:"athlete_id"`
	ActivityID   int64     `json:"activity_id"`
	Attempts     int       `json:"attempts"`
	UpdatedAt    time.Time `json:"updated_at"`
	LastError    *string   `json:"last_error"`
	ArtifactPath *string   `json:"artifact_path"`
	ArtifactHash *string   `json:"artifact_sha256"`
	TraceParent  *string   `json:"traceparent"`
}

func (operations Operations) DeadJobs(ctx context.Context, limit int) ([]DeadJob, error) {
	rows, err := operations.DB.Query(ctx, `SELECT * FROM (
		SELECT 'event' AS kind,e.id,'' AS target,e.owner_id AS athlete_id,e.object_id AS activity_id,
		 e.attempts,e.updated_at,e.last_error,NULL::text AS artifact_path,NULL::text AS artifact_sha256,e.traceparent
		 FROM strava_webhook_events e WHERE e.status='dead'
		UNION ALL
		SELECT 'delivery',d.id,d.target,e.owner_id,e.object_id,d.attempts,d.updated_at,d.last_error,
		 a.relative_path,a.sha256,e.traceparent FROM strava_delivery_outbox d
		 JOIN strava_webhook_events e ON e.id=d.event_id
		 LEFT JOIN gateway_artifacts a ON a.event_id=e.id WHERE d.target='rust' AND d.status='dead'
		UNION ALL
		SELECT 'sync',id,target,athlete_id,0,attempts,updated_at,last_error,NULL,NULL,traceparent
		 FROM gateway_sync_jobs WHERE target='rust' AND status='dead'
	) AS failures ORDER BY updated_at DESC,kind,id DESC LIMIT $1`, min(max(limit, 1), 100))
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	items := []DeadJob{}
	for rows.Next() {
		var item DeadJob
		if err := rows.Scan(&item.Kind, &item.ID, &item.Target, &item.AthleteID, &item.ActivityID,
			&item.Attempts, &item.UpdatedAt, &item.LastError, &item.ArtifactPath, &item.ArtifactHash, &item.TraceParent); err != nil {
			return nil, err
		}
		items = append(items, item)
	}
	return items, rows.Err()
}

func (operations Operations) Counts(ctx context.Context) ([]JobCount, error) {
	rows, err := operations.DB.Query(ctx, `SELECT kind,status,count(*) FROM (
		SELECT 'event' AS kind,status FROM strava_webhook_events
		UNION ALL SELECT 'delivery',status FROM strava_delivery_outbox WHERE target='rust'
		UNION ALL SELECT 'sync',status FROM gateway_sync_jobs WHERE target='rust'
	) AS jobs GROUP BY kind,status ORDER BY kind,status`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	counts := []JobCount{}
	for rows.Next() {
		var item JobCount
		if err := rows.Scan(&item.Kind, &item.Status, &item.Count); err != nil {
			return nil, err
		}
		counts = append(counts, item)
	}
	return counts, rows.Err()
}

func (operations Operations) ReplayDead(ctx context.Context, kind string, id int64) error {
	if id <= 0 {
		return errors.New("job ID must be positive")
	}
	var statement string
	switch kind {
	case "event":
		statement = `UPDATE strava_webhook_events SET status='pending',attempts=0,
			next_attempt_at=now(),lease_until=NULL,updated_at=now()
			WHERE id=$1 AND status='dead'`
	case "delivery":
		statement = `UPDATE strava_delivery_outbox SET status='pending',attempts=0,
			next_attempt_at=now(),lease_until=NULL,last_error=NULL,updated_at=now()
			WHERE id=$1 AND target='rust' AND status='dead'`
	case "sync":
		statement = `UPDATE gateway_sync_jobs SET status='queued',attempts=0,
			next_attempt_at=now(),lease_until=NULL,last_error=NULL,updated_at=now()
			WHERE id=$1 AND target='rust' AND status='dead'`
	default:
		return fmt.Errorf("unknown job kind %q", kind)
	}
	command, err := operations.DB.Exec(ctx, statement, id)
	if err != nil {
		return err
	}
	if command.RowsAffected() != 1 {
		return ErrDeadJobNotFound
	}
	return nil
}
