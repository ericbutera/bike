package worker

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/ericbutera/bike/strava-gateway/internal/observability"
	"github.com/ericbutera/bike/strava-gateway/internal/provider"
	"github.com/ericbutera/bike/strava-gateway/internal/secret"
	"github.com/ericbutera/bike/strava-gateway/internal/storage"
	"github.com/ericbutera/bike/strava-gateway/internal/webhook"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"
)

type rideProviderFake struct {
	provider.SyncAPI
	activity, streams json.RawMessage
}

func (fake *rideProviderFake) FetchActivity(context.Context, string, int64) (provider.Response, error) {
	return provider.Response{StatusCode: http.StatusOK, Body: fake.activity}, nil
}

func (fake *rideProviderFake) FetchStreams(context.Context, string, int64) (provider.Response, error) {
	return provider.Response{StatusCode: http.StatusOK, Body: fake.streams}, nil
}

func failureFixtureDB(t *testing.T) *pgxpool.Pool {
	t.Helper()
	databaseURL := os.Getenv("TEST_DATABASE_URL")
	if databaseURL == "" {
		t.Skip("TEST_DATABASE_URL is not set; explicit SQL functional test")
	}
	ctx := context.Background()
	admin, err := pgxpool.New(ctx, databaseURL)
	if err != nil {
		t.Fatal(err)
	}
	schema := "failure_fixture_" + strconv.FormatInt(time.Now().UnixNano(), 10)
	quoted := pgx.Identifier{schema}.Sanitize()
	if _, err := admin.Exec(ctx, "CREATE SCHEMA "+quoted); err != nil {
		admin.Close()
		t.Fatal(err)
	}
	t.Cleanup(func() { _, _ = admin.Exec(ctx, "DROP SCHEMA "+quoted+" CASCADE"); admin.Close() })
	parsed, err := url.Parse(databaseURL)
	if err != nil {
		t.Fatal(err)
	}
	query := parsed.Query()
	query.Set("search_path", schema)
	parsed.RawQuery = query.Encode()
	if err := storage.Migrate(ctx, parsed.String()); err != nil {
		t.Fatal(err)
	}
	db, err := pgxpool.New(ctx, parsed.String())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(db.Close)
	return db
}

func TestMalformedStreamsAreRetainedAndReplayWithRealRideFixture(t *testing.T) {
	db := failureFixtureDB(t)
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	body, err := os.ReadFile("../provider/testdata/strava-real-ride.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixture struct{ Activity, Streams json.RawMessage }
	if err := json.Unmarshal(body, &fixture); err != nil {
		t.Fatal(err)
	}
	activity, err := provider.ParseActivity(fixture.Activity)
	if err != nil {
		t.Fatal(err)
	}
	cipher, err := secret.NewTokenCipher(base64.StdEncoding.EncodeToString(bytes.Repeat([]byte{7}, 32)))
	if err != nil {
		t.Fatal(err)
	}
	connections := storage.Connections{DB: db, Cipher: cipher}
	if err := connections.Upsert(ctx, storage.Connection{AthleteID: 8, AccessToken: "private-access", RefreshToken: "private-refresh", ExpiresAt: time.Now().Add(time.Hour), Scopes: []string{"activity:read_all"}}); err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec(ctx, `INSERT INTO gateway_site_links (athlete_id,target,site_user_id) VALUES (8,'rust',1)`); err != nil {
		t.Fatal(err)
	}
	event := webhook.Event{AspectType: "create", EventTime: time.Now().Unix(), ObjectID: activity.ID, ObjectType: "activity", OwnerID: 8, SubscriptionID: 12}
	raw, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := (storage.Inbox{DB: db, Targets: []string{"rust"}}).Store(ctx, event, raw); err != nil {
		t.Fatal(err)
	}
	malformed := json.RawMessage("{unsupported streams\xff")
	fake := &rideProviderFake{activity: fixture.Activity, streams: malformed}
	var logs strings.Builder
	worker := Worker{Jobs: storage.Jobs{DB: db}, Syncs: storage.Syncs{DB: db}, Connections: connections,
		Quota: storage.Quota{DB: db}, Provider: fake, Artifacts: Artifacts{Root: t.TempDir()},
		Metrics: observability.NewMetrics(), Logger: observability.NewLogger("test-worker", &logs)}
	if worked, err := worker.Tick(ctx); err != nil || !worked {
		t.Fatalf("failed payload tick: %t, %v", worked, err)
	}
	operator := storage.Operations{DB: db}
	failures, err := operator.DeadJobs(ctx, 50)
	if err != nil || len(failures) != 1 || failures[0].Kind != "event" || failures[0].Attempts != 1 || failures[0].LastError == nil {
		t.Fatalf("retained failure: %+v, %v", failures, err)
	}
	if !strings.Contains(*failures[0].LastError, "sha256=") {
		t.Fatal("dead job is missing its diagnostic artifact reference")
	}
	files, err := filepath.Glob(filepath.Join(worker.Artifacts.Root, "*", "*.json"))
	if err != nil || len(files) != 1 {
		t.Fatalf("diagnostic artifacts: %v, %v", files, err)
	}
	diagnostic, err := os.ReadFile(files[0])
	if err != nil {
		t.Fatal(err)
	}
	hash := strings.TrimSuffix(filepath.Base(files[0]), ".json")
	exported, err := worker.Artifacts.ReadByHash(hash)
	if err != nil || !bytes.Equal(exported, diagnostic) {
		t.Fatalf("operator artifact export differs: %v", err)
	}
	var capture struct {
		Stage string
		Body  []byte `json:"body_base64"`
	}
	if err := json.Unmarshal(diagnostic, &capture); err != nil {
		t.Fatal(err)
	}
	if capture.Stage != "streams" || !bytes.Equal(capture.Body, malformed) {
		t.Fatal("diagnostic did not retain the exact malformed body")
	}
	if !strings.Contains(logs.String(), `"disposition":"dead"`) || !strings.Contains(logs.String(), `"job_id":`) || strings.Contains(logs.String(), "private-access") || strings.Contains(logs.String(), "unsupported streams") {
		t.Fatalf("unexpected diagnostic log: %s", logs.String())
	}
	// A fresh registry must discover the persisted failure after a worker restart.
	worker.Metrics = observability.NewMetrics()
	worker.refreshMetrics(ctx)
	assertDeadLetterMetric(t, worker, 1)
	fake.streams = fixture.Streams
	if err := operator.ReplayDead(ctx, "event", failures[0].ID); err != nil {
		t.Fatal(err)
	}
	if worked, err := worker.Tick(ctx); err != nil || !worked {
		t.Fatalf("successful replay: %t, %v", worked, err)
	}
	worker.refreshMetrics(ctx)
	assertDeadLetterMetric(t, worker, 0)
	delivery, err := worker.Jobs.ClaimDelivery(ctx)
	if err != nil || delivery == nil || delivery.Artifact == nil {
		t.Fatalf("replay did not produce a valid activity delivery: %+v, %v", delivery, err)
	}
	if failures, err := operator.DeadJobs(ctx, 50); err != nil || len(failures) != 0 {
		t.Fatalf("unresolved failures after replay: %+v, %v", failures, err)
	}
}

func assertDeadLetterMetric(t *testing.T, worker Worker, want int) {
	t.Helper()
	recorder := httptest.NewRecorder()
	worker.Metrics.Handler().ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, "/metrics", nil))
	expected := `bike_strava_gateway_dead_letter_jobs{queue="webhook"} ` + strconv.Itoa(want)
	if !strings.Contains(recorder.Body.String(), expected+"\n") {
		t.Fatalf("missing persistent signal %s", expected)
	}
}
