package storage

import (
	"context"
	"encoding/json"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/ericbutera/bike/strava-gateway/internal/webhook"
	"github.com/jackc/pgx/v5/pgxpool"
)

type reconciliationHistory struct {
	status    string
	operation string
	hash      string
}

func TestReconciliationSkipsOnlyUnchangedDeliveredContent(t *testing.T) {
	pool, ctx := reconciliationDatabase(t)
	hashA, hashB := strings.Repeat("a", 64), strings.Repeat("b", 64)
	cases := []struct {
		name        string
		history     []reconciliationHistory
		webhook     bool
		manual      bool
		reconnected bool
		want        string
	}{
		{name: "first delivery", want: "pending"},
		{name: "unchanged", history: []reconciliationHistory{{"delivered", "upsert", hashA}}, want: "unchanged"},
		{name: "repeated unchanged", history: []reconciliationHistory{{"delivered", "upsert", hashA}, {"unchanged", "upsert", hashA}}, want: "unchanged"},
		{name: "changed", history: []reconciliationHistory{{"delivered", "upsert", hashB}}, want: "pending"},
		{name: "reverted", history: []reconciliationHistory{{"delivered", "upsert", hashA}, {"delivered", "upsert", hashB}}, want: "pending"},
		{name: "pending", history: []reconciliationHistory{{"delivered", "upsert", hashA}, {"pending", "upsert", hashA}}, want: "pending"},
		{name: "failed", history: []reconciliationHistory{{"delivered", "upsert", hashA}, {"dead", "upsert", hashA}}, want: "pending"},
		{name: "deleted", history: []reconciliationHistory{{"delivered", "upsert", hashA}, {"delivered", "delete", ""}}, want: "pending"},
		{name: "deauthorized", history: []reconciliationHistory{{"delivered", "upsert", hashA}, {"delivered", "deauthorize", ""}}, want: "pending"},
		{name: "webhook", history: []reconciliationHistory{{"delivered", "upsert", hashA}}, webhook: true, want: "pending"},
		{name: "manual sync", history: []reconciliationHistory{{"delivered", "upsert", hashA}}, manual: true, want: "pending"},
		{name: "reconnected", history: []reconciliationHistory{{"delivered", "upsert", hashA}}, reconnected: true, want: "pending"},
	}
	for _, item := range cases {
		t.Run(item.name, func(t *testing.T) {
			resetReconciliationLink(t, ctx, pool)
			for index, prior := range item.history {
				seedReconciliationDelivery(t, ctx, pool, index+1, prior)
			}
			if item.reconnected {
				if _, err := pool.Exec(ctx, `UPDATE gateway_site_links SET updated_at=now() WHERE athlete_id=14`); err != nil {
					t.Fatal(err)
				}
			}
			event := webhook.Event{OwnerID: 14, ObjectID: 42, ObjectType: "activity", AspectType: "update",
				EventTime: 100, Reconciliation: !item.manual}
			if item.webhook {
				event.SubscriptionID = 12 // Even a forged reconciliation flag cannot suppress a real webhook.
			}
			assertReconciliationCompletion(t, ctx, pool, event, hashA, item.want)
		})
	}
}

func reconciliationDatabase(t *testing.T) (*pgxpool.Pool, context.Context) {
	t.Helper()
	url := os.Getenv("TEST_DATABASE_URL")
	if url == "" {
		t.Skip("TEST_DATABASE_URL is not set")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	t.Cleanup(cancel)
	for range 2 {
		if err := Migrate(ctx, url); err != nil {
			t.Fatal(err)
		}
	}
	pool, err := pgxpool.New(ctx, url)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(pool.Close)
	t.Cleanup(func() { resetGatewayFixture(t, context.Background(), pool, 14) })
	return pool, ctx
}

func resetReconciliationLink(t *testing.T, ctx context.Context, pool *pgxpool.Pool) {
	t.Helper()
	resetGatewayFixture(t, ctx, pool, 14)
	if _, err := pool.Exec(ctx, `INSERT INTO gateway_connections (athlete_id,token_ciphertext,expires_at,scopes)
		VALUES (14,$1,now()+interval '1 day',ARRAY['activity:read_all'])`, []byte{1}); err != nil {
		t.Fatal(err)
	}
	if _, err := pool.Exec(ctx, `INSERT INTO gateway_site_links (athlete_id,target,site_user_id,updated_at)
		VALUES (14,'rust',14,now()-interval '1 day')`); err != nil {
		t.Fatal(err)
	}
}

func seedReconciliationDelivery(t *testing.T, ctx context.Context, pool *pgxpool.Pool, sequence int, prior reconciliationHistory) {
	t.Helper()
	objectType, objectID := "activity", int64(42)
	if prior.operation == "deauthorize" {
		objectType, objectID = "athlete", 14
	}
	if _, err := pool.Exec(ctx, `WITH event AS (
		INSERT INTO strava_webhook_events (event_key,subscription_id,owner_id,object_id,object_type,aspect_type,event_time,payload,status)
		VALUES ($1,0,14,$2,$3,'update',$4,'{}','processed') RETURNING id
	) INSERT INTO strava_delivery_outbox (event_id,target,status,operation,artifact_sha256)
	SELECT id,'rust',$5,$6,NULLIF($7,'') FROM event`,
		strings.Repeat(string(rune('c'+sequence)), 64), objectID, objectType, sequence, prior.status, prior.operation, prior.hash); err != nil {
		t.Fatal(err)
	}
}

func assertReconciliationCompletion(t *testing.T, ctx context.Context, pool *pgxpool.Pool, event webhook.Event, hash, want string) {
	t.Helper()
	raw, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}
	if inserted, err := (Inbox{DB: pool, Targets: []string{"rust"}}).Store(ctx, event, raw); err != nil || !inserted {
		t.Fatalf("store reconciliation: %t, %v", inserted, err)
	}
	jobs := Jobs{DB: pool}
	job, err := jobs.ClaimEvent(ctx)
	if err != nil || job == nil {
		t.Fatalf("claim reconciliation: %+v, %v", job, err)
	}
	artifact := Artifact{SHA256: hash, RelativePath: "fixture.json", SizeBytes: 123}
	if err := jobs.CompleteEvent(ctx, *job, "upsert", &artifact); err != nil {
		t.Fatal(err)
	}
	var status, retainedHash, eventStatus string
	var attempts int
	if err := pool.QueryRow(ctx, `SELECT delivery.status,delivery.artifact_sha256,delivery.attempts,event.status
		FROM strava_delivery_outbox delivery JOIN strava_webhook_events event ON event.id=delivery.event_id
		WHERE event.id=$1`, job.ID).Scan(&status, &retainedHash, &attempts, &eventStatus); err != nil {
		t.Fatal(err)
	}
	if status != want || retainedHash != hash || attempts != 0 || eventStatus != "processed" {
		t.Fatalf("completion: status=%s hash=%s attempts=%d event=%s; want status=%s with retained hash and no delivery attempt",
			status, retainedHash, attempts, eventStatus, want)
	}
	if want == "unchanged" {
		var claimable int
		if err := pool.QueryRow(ctx, `SELECT count(*) FROM strava_delivery_outbox WHERE event_id=$1
			AND status IN ('pending','processing')`, job.ID).Scan(&claimable); err != nil || claimable != 0 {
			t.Fatalf("unchanged delivery must never be sent: %d, %v", claimable, err)
		}
	}
}
