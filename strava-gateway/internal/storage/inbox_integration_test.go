package storage

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"errors"
	"os"
	"testing"
	"time"

	"github.com/ericbutera/bike/strava-gateway/internal/secret"
	"github.com/ericbutera/bike/strava-gateway/internal/webhook"
	"github.com/jackc/pgx/v5/pgxpool"
)

func TestMigrationAndWebhookOutbox(t *testing.T) {
	databaseURL := os.Getenv("TEST_DATABASE_URL")
	if databaseURL == "" {
		t.Skip("TEST_DATABASE_URL is not set")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	for range 2 {
		if err := Migrate(ctx, databaseURL); err != nil {
			t.Fatalf("migration must be repeatable: %v", err)
		}
	}
	pool, err := pgxpool.New(ctx, databaseURL)
	if err != nil {
		t.Fatal(err)
	}
	defer pool.Close()
	resetGatewayFixture(t, ctx, pool, 8, 9, 10)
	defer resetGatewayFixture(t, context.Background(), pool, 8)
	if _, err := pool.Exec(ctx, `INSERT INTO gateway_connections
		(athlete_id,token_ciphertext,expires_at,scopes)
		VALUES (8,$1,now()+interval '1 day',ARRAY['activity:read_all'])
		ON CONFLICT (athlete_id) DO NOTHING`, []byte{1}); err != nil {
		t.Fatal(err)
	}
	if _, err := pool.Exec(ctx, `INSERT INTO gateway_site_links
		(athlete_id,target,site_user_id) VALUES (8,'rust',1)
		ON CONFLICT (athlete_id,target) DO NOTHING`); err != nil {
		t.Fatal(err)
	}
	event := webhook.Event{
		AspectType: "create", EventTime: time.Now().UnixNano(), ObjectID: 42,
		ObjectType: "activity", OwnerID: 8, SubscriptionID: 12,
	}
	raw, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}
	inbox := Inbox{DB: pool, Targets: []string{"rust"}}
	inserted, err := inbox.Store(ctx, event, raw)
	if err != nil {
		t.Fatalf("store event: %v", err)
	}
	if !inserted {
		t.Fatal("first event store was reported as duplicate")
	}
	inserted, err = inbox.Store(ctx, event, raw)
	if err != nil {
		t.Fatalf("store duplicate: %v", err)
	}
	if inserted {
		t.Fatal("duplicate event store was reported as inserted")
	}
	var eventCount, deliveryCount int
	if err := pool.QueryRow(ctx, `SELECT count(*) FROM strava_webhook_events WHERE event_key=$1`, event.Key()).Scan(&eventCount); err != nil {
		t.Fatal(err)
	}
	if err := pool.QueryRow(ctx, `SELECT count(*) FROM strava_delivery_outbox d
		JOIN strava_webhook_events e ON e.id=d.event_id WHERE e.event_key=$1`, event.Key()).Scan(&deliveryCount); err != nil {
		t.Fatal(err)
	}
	if eventCount != 1 || deliveryCount != 1 {
		t.Fatalf("got %d event rows and %d delivery rows; want 1 and 1", eventCount, deliveryCount)
	}
	jobs := Jobs{DB: pool}
	claimed, err := jobs.ClaimEvent(ctx)
	if err != nil || claimed == nil {
		t.Fatalf("claim event: %v, %v", claimed, err)
	}
	artifact := Artifact{SHA256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
		RelativePath: "aa/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.json", SizeBytes: 123}
	if err := jobs.CompleteEvent(ctx, *claimed, "upsert", &artifact); err != nil {
		t.Fatalf("complete fetched event: %v", err)
	}
	for range 1 {
		delivery, err := jobs.ClaimDelivery(ctx)
		if err != nil || delivery == nil {
			t.Fatalf("claim delivery: %v, %v", delivery, err)
		}
		if delivery.Artifact == nil || delivery.Artifact.SHA256 != artifact.SHA256 {
			t.Fatalf("delivery artifact: %+v", delivery.Artifact)
		}
		if err := jobs.CompleteDelivery(ctx, *delivery); err != nil {
			t.Fatalf("complete delivery: %v", err)
		}
	}
	var replayID int64
	if err := pool.QueryRow(ctx, `UPDATE strava_delivery_outbox AS deliveries
		SET status='dead' FROM strava_webhook_events AS events
		WHERE deliveries.event_id=events.id AND events.event_key=$1
		  AND deliveries.target='rust' RETURNING deliveries.id`, event.Key()).Scan(&replayID); err != nil {
		t.Fatal(err)
	}
	operator := Operations{DB: pool}
	failures, err := operator.DeadJobs(ctx, 50)
	if err != nil || len(failures) != 1 || failures[0].ID != replayID || failures[0].ArtifactHash == nil || *failures[0].ArtifactHash != artifact.SHA256 {
		t.Fatalf("retained delivery failure: %+v, %v", failures, err)
	}
	stats, err := jobs.QueueStats(ctx)
	if err != nil || stats.Dead != [3]int64{0, 1, 0} {
		t.Fatalf("dead delivery signal: %+v, %v", stats, err)
	}
	if err := operator.ReplayDead(ctx, "delivery", replayID); err != nil {
		t.Fatal(err)
	}
	if err := operator.ReplayDead(ctx, "delivery", replayID); !errors.Is(err, ErrDeadJobNotFound) {
		t.Fatalf("replaying a non-dead delivery returned %v", err)
	}
	stats, err = jobs.QueueStats(ctx)
	if err != nil || stats.Dead != [3]int64{} {
		t.Fatalf("replay must clear the dead-letter signal: %+v, %v", stats, err)
	}
	counts, err := operator.Counts(ctx)
	if err != nil || len(counts) == 0 {
		t.Fatalf("operator counts: %v, %v", counts, err)
	}
	if err := jobs.RequeueMissingArtifact(ctx, DeliveryJob{
		EventID: claimed.ID, Operation: "upsert",
	}); err != nil {
		t.Fatal(err)
	}
	var eventStatus, deliveryStatus string
	if err := pool.QueryRow(ctx, `SELECT status FROM strava_webhook_events WHERE id=$1`,
		claimed.ID).Scan(&eventStatus); err != nil {
		t.Fatal(err)
	}
	if err := pool.QueryRow(ctx, `SELECT status FROM strava_delivery_outbox WHERE id=$1`,
		replayID).Scan(&deliveryStatus); err != nil {
		t.Fatal(err)
	}
	if eventStatus != "pending" || deliveryStatus != "waiting_for_fetch" {
		t.Fatalf("refetch states: event=%q delivery=%q", eventStatus, deliveryStatus)
	}
}

func TestRustImportPreservesExistingGatewayTokenAndMapping(t *testing.T) {
	databaseURL := os.Getenv("TEST_DATABASE_URL")
	if databaseURL == "" {
		t.Skip("TEST_DATABASE_URL is not set")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	if err := Migrate(ctx, databaseURL); err != nil {
		t.Fatal(err)
	}
	pool, err := pgxpool.New(ctx, databaseURL)
	if err != nil {
		t.Fatal(err)
	}
	defer pool.Close()
	resetGatewayFixture(t, ctx, pool, 11)
	defer resetGatewayFixture(t, context.Background(), pool, 11)
	key := base64.StdEncoding.EncodeToString(bytes.Repeat([]byte{7}, 32))
	cipher, err := secret.NewTokenCipher(key)
	if err != nil {
		t.Fatal(err)
	}
	connections := Connections{DB: pool, Cipher: cipher}
	item := Connection{AthleteID: 11, AccessToken: "original-access", RefreshToken: "original-refresh",
		ExpiresAt: time.Now().Add(time.Hour), Scopes: []string{"activity:read_all"}}
	if created, err := connections.ImportRust(ctx, item, 11, false); err != nil || !created {
		t.Fatalf("initial import: %t, %v", created, err)
	}
	item.AccessToken = "stale-rust-access"
	if created, err := connections.ImportRust(ctx, item, 11, false); err != nil || created {
		t.Fatalf("idempotent import: %t, %v", created, err)
	}
	stored, err := connections.Find(ctx, 11)
	if err != nil || stored.AccessToken != "original-access" {
		t.Fatalf("existing gateway token was overwritten: %v", err)
	}
	if _, err := connections.ImportRust(ctx, item, 12, false); !errors.Is(err, ErrLinkConflict) {
		t.Fatalf("conflicting Rust user accepted: %v", err)
	}
	if written, err := connections.ImportRust(ctx, item, 11, true); err != nil || !written {
		t.Fatalf("explicit handoff failed: %t, %v", written, err)
	}
	stored, err = connections.Find(ctx, 11)
	if err != nil || stored.AccessToken != "stale-rust-access" {
		t.Fatalf("explicit handoff did not replace token: %v", err)
	}
}

func TestReconciliationQueuesEachEnabledLinkAtMostOncePerWindow(t *testing.T) {
	databaseURL := os.Getenv("TEST_DATABASE_URL")
	if databaseURL == "" {
		t.Skip("TEST_DATABASE_URL is not set")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	if err := Migrate(ctx, databaseURL); err != nil {
		t.Fatal(err)
	}
	pool, err := pgxpool.New(ctx, databaseURL)
	if err != nil {
		t.Fatal(err)
	}
	defer pool.Close()
	const athleteID = 10
	resetGatewayFixture(t, ctx, pool, athleteID)
	defer resetGatewayFixture(t, context.Background(), pool, athleteID)
	if _, err := pool.Exec(ctx, `INSERT INTO gateway_connections
		(athlete_id,token_ciphertext,expires_at,scopes)
		VALUES ($1,$2,now()+interval '1 day',ARRAY['activity:read_all'])
		ON CONFLICT (athlete_id) DO NOTHING`, athleteID, []byte{1}); err != nil {
		t.Fatal(err)
	}
	if _, err := pool.Exec(ctx, `INSERT INTO gateway_site_links
		(athlete_id,target,site_user_id,enabled) VALUES ($1,'rust',10,true)
		ON CONFLICT (athlete_id,target) DO UPDATE SET enabled=true`, athleteID); err != nil {
		t.Fatal(err)
	}
	syncs := Syncs{DB: pool}
	now := time.Now()
	if count, err := syncs.QueueDueReconciliation(ctx, now); err != nil || count < 1 {
		t.Fatalf("first reconciliation queued %d links: %v", count, err)
	}
	if count, err := syncs.QueueDueReconciliation(ctx, now); err != nil || count != 0 {
		t.Fatalf("duplicate reconciliation queued %d links: %v", count, err)
	}
	var jobs int
	if err := pool.QueryRow(ctx, `SELECT count(*) FROM gateway_sync_jobs
		WHERE athlete_id=$1 AND target='rust'`, athleteID).Scan(&jobs); err != nil || jobs != 1 {
		t.Fatalf("reconciliation jobs: %d, %v", jobs, err)
	}
}

func TestQuotaReservationsAreSharedAndBlockWhenExhausted(t *testing.T) {
	databaseURL := os.Getenv("TEST_DATABASE_URL")
	if databaseURL == "" {
		t.Skip("TEST_DATABASE_URL is not set")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	if err := Migrate(ctx, databaseURL); err != nil {
		t.Fatal(err)
	}
	pool, err := pgxpool.New(ctx, databaseURL)
	if err != nil {
		t.Fatal(err)
	}
	defer pool.Close()
	if _, err := pool.Exec(ctx, `DELETE FROM gateway_rate_limits`); err != nil {
		t.Fatal(err)
	}
	quota := Quota{DB: pool, Limits: [4]int{1, 5, 1, 5}}
	if retry, err := quota.Acquire(ctx); err != nil || !retry.IsZero() {
		t.Fatalf("first reservation: %s, %v", retry, err)
	}
	otherWorker := Quota{DB: pool, Limits: quota.Limits}
	if retry, err := otherWorker.Acquire(ctx); err != nil || !retry.After(time.Now()) {
		t.Fatalf("exhausted reservation: %s, %v", retry, err)
	}
	var used int
	if err := pool.QueryRow(ctx, `SELECT used_count FROM gateway_rate_limits WHERE bucket='overall_15m'`).Scan(&used); err != nil {
		t.Fatal(err)
	}
	if used != 1 {
		t.Fatalf("exhausted quota consumed %d requests", used)
	}
	blocked, err := quota.BlockedBuckets(ctx)
	if err != nil || len(blocked) != 2 || blocked[0] != "nonupload_15m" || blocked[1] != "overall_15m" {
		t.Fatalf("short-window exhausted buckets = %v, %v", blocked, err)
	}
}

func TestSyncPageQueuesTargetDeliveryAndCompletes(t *testing.T) {
	databaseURL := os.Getenv("TEST_DATABASE_URL")
	if databaseURL == "" {
		t.Skip("TEST_DATABASE_URL is not set")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	if err := Migrate(ctx, databaseURL); err != nil {
		t.Fatal(err)
	}
	pool, err := pgxpool.New(ctx, databaseURL)
	if err != nil {
		t.Fatal(err)
	}
	defer pool.Close()
	resetGatewayFixture(t, ctx, pool, 9)
	defer resetGatewayFixture(t, context.Background(), pool, 9)
	if _, err := pool.Exec(ctx, `INSERT INTO gateway_connections
		(athlete_id,token_ciphertext,expires_at,scopes)
		VALUES (9,$1,now()+interval '1 day',ARRAY['activity:read_all'])
		ON CONFLICT (athlete_id) DO NOTHING`, []byte{1}); err != nil {
		t.Fatal(err)
	}
	if _, err := pool.Exec(ctx, `INSERT INTO gateway_site_links
		(athlete_id,target,site_user_id) VALUES (9,'rust',1)
		ON CONFLICT (athlete_id,target) DO NOTHING`); err != nil {
		t.Fatal(err)
	}
	syncs := Syncs{DB: pool}
	link := SiteLink{AthleteID: 9, Target: "rust", UserID: 1}
	if err := syncs.Queue(ctx, link, "incremental"); err != nil {
		t.Fatal(err)
	}
	if err := syncs.Queue(ctx, link, "initial"); err != nil {
		t.Fatal(err)
	}
	job, err := syncs.Claim(ctx)
	if err != nil || job == nil {
		t.Fatalf("claim sync: %v, %v", job, err)
	}
	if job.AthleteID != 9 || job.Target != "rust" || job.Page != 1 || job.AfterEpoch <= 0 {
		t.Fatalf("unexpected sync job: %+v", job)
	}
	if err := syncs.Fail(ctx, *job, "unsupported activity list"); err != nil {
		t.Fatal(err)
	}
	operator := Operations{DB: pool}
	failures, err := operator.DeadJobs(ctx, 50)
	if err != nil || len(failures) != 1 || failures[0].Kind != "sync" || failures[0].LastError == nil || *failures[0].LastError != "unsupported activity list" {
		t.Fatalf("retained sync failure: %+v, %v", failures, err)
	}
	stats, err := (Jobs{DB: pool}).QueueStats(ctx)
	if err != nil || stats.Dead != [3]int64{0, 0, 1} {
		t.Fatalf("dead sync signal: %+v, %v", stats, err)
	}
	if err := operator.ReplayDead(ctx, "sync", job.ID); err != nil {
		t.Fatal(err)
	}
	job, err = syncs.Claim(ctx)
	if err != nil || job == nil {
		t.Fatalf("claim replayed sync: %+v, %v", job, err)
	}
	afterEpoch := job.AfterEpoch
	retryAt := time.Now().UTC().Add(time.Minute)
	if err := syncs.Retry(ctx, *job, retryAt, "rate limit", "short_window_quota"); err != nil {
		t.Fatal(err)
	}
	status, err := syncs.Status(ctx, link)
	if err != nil || status.Status != "queued" || status.WaitingReason == nil || *status.WaitingReason != "short_window_quota" || status.NextAttemptAt == nil {
		t.Fatalf("sync pause detail: %+v, %v", status, err)
	}
	if delta := status.NextAttemptAt.Sub(retryAt); delta < -time.Second || delta > time.Second {
		t.Fatalf("next attempt = %s, want %s", status.NextAttemptAt, retryAt)
	}
	if _, err := pool.Exec(ctx, `UPDATE gateway_sync_jobs SET created_at=now()-interval '1 hour' WHERE id=$1`, job.ID); err != nil {
		t.Fatal(err)
	}
	stats, err = (Jobs{DB: pool}).QueueStats(ctx)
	if err != nil || stats.Syncs.OldestAge != 0 || stats.Dead != [3]int64{} {
		t.Fatalf("expected quota pause must not look stuck or dead: %+v, %v", stats, err)
	}
	if _, err := pool.Exec(ctx, `UPDATE gateway_sync_jobs SET next_attempt_at=now() WHERE id=$1`, job.ID); err != nil {
		t.Fatal(err)
	}
	job, err = syncs.Claim(ctx)
	if err != nil || job == nil {
		t.Fatalf("reclaim sync: %v, %v", job, err)
	}
	if job.Page != 1 || job.AfterEpoch != afterEpoch || job.WaitingReason != "short_window_quota" {
		t.Fatalf("pause resume lost sync checkpoint: %+v", job)
	}
	if _, err := pool.Exec(ctx, `UPDATE gateway_sync_jobs SET lease_until=now()-interval '1 second' WHERE id=$1`, job.ID); err != nil {
		t.Fatal(err)
	}
	job, err = syncs.Claim(ctx)
	if err != nil || job == nil {
		t.Fatalf("reclaim expired sync lease: %v, %v", job, err)
	}
	if job.Page != 1 || job.AfterEpoch != afterEpoch || job.WaitingReason != "" {
		t.Fatalf("expired lease changed sync checkpoint or repeated pause: %+v", job)
	}
	if err := syncs.CompletePage(ctx, *job, []int64{123}); err != nil {
		t.Fatal(err)
	}
	status, err = syncs.Status(ctx, link)
	if err != nil || status.Status != "succeeded" || status.WaitingReason != nil || status.NextAttemptAt != nil {
		t.Fatalf("sync status: %+v, %v", status, err)
	}
	var targets []string
	rows, err := pool.Query(ctx, `SELECT d.target FROM strava_delivery_outbox d
		JOIN strava_webhook_events e ON e.id=d.event_id
		WHERE e.owner_id=9 AND e.object_id=123`)
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	for rows.Next() {
		var target string
		if err := rows.Scan(&target); err != nil {
			t.Fatal(err)
		}
		targets = append(targets, target)
	}
	if err := rows.Err(); err != nil {
		t.Fatal(err)
	}
	if len(targets) != 1 || targets[0] != "rust" {
		t.Fatalf("sync queued targets %v; want only rust", targets)
	}
	var pauseEvents []string
	eventRows, err := pool.Query(ctx, `SELECT event_type || ':' || waiting_reason FROM gateway_sync_events
		WHERE sync_job_id=$1 ORDER BY id`, job.ID)
	if err != nil {
		t.Fatal(err)
	}
	defer eventRows.Close()
	for eventRows.Next() {
		var eventType string
		if err := eventRows.Scan(&eventType); err != nil {
			t.Fatal(err)
		}
		pauseEvents = append(pauseEvents, eventType)
	}
	if err := eventRows.Err(); err != nil {
		t.Fatal(err)
	}
	if len(pauseEvents) != 2 || pauseEvents[0] != "paused:short_window_quota" || pauseEvents[1] != "resumed:short_window_quota" {
		t.Fatalf("sync pause/resume events = %v, want short-window pause and resume", pauseEvents)
	}
	if err := syncs.Queue(ctx, link, "incremental"); err != nil {
		t.Fatal(err)
	}
	pausedJob, err := syncs.Claim(ctx)
	if err != nil || pausedJob == nil {
		t.Fatalf("claim sync for disconnect: %v, %v", pausedJob, err)
	}
	if err := syncs.Retry(ctx, *pausedJob, time.Now().UTC().Add(time.Minute), "rate limit", "daily_quota"); err != nil {
		t.Fatal(err)
	}
	if err := (Connections{DB: pool}).DisableLink(ctx, link); err != nil {
		t.Fatal(err)
	}
	status, err = syncs.Status(ctx, link)
	if err != nil || status.Status != "succeeded" || status.WaitingReason != nil || status.NextAttemptAt != nil {
		t.Fatalf("disconnect left paused sync active: %+v, %v", status, err)
	}
	if job, err := syncs.Claim(ctx); err != nil || job != nil {
		t.Fatalf("disconnected sync was claimable: %+v, %v", job, err)
	}
}

func resetGatewayFixture(t *testing.T, ctx context.Context, pool *pgxpool.Pool, athleteIDs ...int) {
	t.Helper()
	for _, statement := range []string{
		`DELETE FROM gateway_sync_jobs WHERE athlete_id=ANY($1)`,
		`DELETE FROM strava_webhook_events WHERE owner_id=ANY($1)`,
		`DELETE FROM gateway_site_links WHERE athlete_id=ANY($1)`,
		`DELETE FROM gateway_connections WHERE athlete_id=ANY($1)`,
	} {
		if _, err := pool.Exec(ctx, statement, athleteIDs); err != nil {
			t.Fatal(err)
		}
	}
}
