package oauth

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"strconv"
	"testing"
	"time"

	"github.com/ericbutera/bike/strava-gateway/internal/provider"
	"github.com/ericbutera/bike/strava-gateway/internal/secret"
	"github.com/ericbutera/bike/strava-gateway/internal/storage"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"
)

type fixtureOAuth struct {
	token provider.Token
	codes []string
}

func (fake *fixtureOAuth) AuthorizationURL(string, string) string {
	return "https://strava.test/authorize"
}

func (fake *fixtureOAuth) ExchangeCode(_ context.Context, code string) (provider.Token, provider.Response, error) {
	fake.codes = append(fake.codes, code)
	return fake.token, provider.Response{}, nil
}

func (fake *fixtureOAuth) AuthenticatedAthlete(context.Context, string) (int64, provider.Response, error) {
	return fake.token.Athlete.ID, provider.Response{}, nil
}

func TestCallbackBindsSiteUserAndQueuesInitialSyncWithFakeProvider(t *testing.T) {
	databaseURL := os.Getenv("TEST_DATABASE_URL")
	if databaseURL == "" {
		t.Skip("TEST_DATABASE_URL is not set; this is an explicitly selected functional test")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	pool, err := pgxpool.New(ctx, databaseURL)
	if err != nil {
		t.Fatal(err)
	}
	defer pool.Close()
	schema := "oauth_fixture_" + strconv.FormatInt(time.Now().UnixNano(), 10)
	quotedSchema := pgx.Identifier{schema}.Sanitize()
	if _, err := pool.Exec(ctx, "CREATE SCHEMA "+quotedSchema); err != nil {
		t.Fatal(err)
	}
	defer func() {
		if _, err := pool.Exec(context.Background(), "DROP SCHEMA "+quotedSchema+" CASCADE"); err != nil {
			t.Errorf("drop fixture schema: %v", err)
		}
	}()
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
	fixtureDB, err := pgxpool.New(ctx, parsed.String())
	if err != nil {
		t.Fatal(err)
	}
	defer fixtureDB.Close()
	cipher, err := secret.NewTokenCipher(base64.StdEncoding.EncodeToString(bytes.Repeat([]byte{7}, 32)))
	if err != nil {
		t.Fatal(err)
	}
	body, err := os.ReadFile("../provider/testdata/token-exchange.json")
	if err != nil {
		t.Fatal(err)
	}
	fake := &fixtureOAuth{}
	if err := json.Unmarshal(body, &fake.token); err != nil {
		t.Fatal(err)
	}
	handler := Handler{States: storage.OAuthStates{DB: fixtureDB},
		Connections: storage.Connections{DB: fixtureDB, Cipher: cipher},
		Syncs:       storage.Syncs{DB: fixtureDB}, Quota: storage.Quota{DB: fixtureDB}, Provider: fake,
		Sites: map[string]Site{"rust": {ReturnURL: "https://bike.test/settings"}}}
	if err := handler.States.Save(ctx, "fixture-state", storage.OAuthState{Target: "rust", UserID: 17}); err != nil {
		t.Fatal(err)
	}
	response := httptest.NewRecorder()
	handler.Callback(response, httptest.NewRequest(http.MethodGet, "/oauth/callback?state=fixture-state&code=fixture-code", nil))
	if response.Code != http.StatusSeeOther || response.Header().Get("Location") != "https://bike.test/settings?strava=connected" {
		t.Fatalf("callback = %d %q", response.Code, response.Header().Get("Location"))
	}
	link, err := handler.Connections.FindLinkByUser(ctx, "rust", 17)
	if err != nil || link.AthleteID != fake.token.Athlete.ID {
		t.Fatalf("site binding = %+v, error = %v", link, err)
	}
	connection, err := handler.Connections.Find(ctx, link.AthleteID)
	if err != nil || connection.RefreshToken != fake.token.RefreshToken {
		t.Fatalf("connection did not retain the fixture credential: %v", err)
	}
	var mode, status string
	var userID, afterEpoch int64
	err = fixtureDB.QueryRow(ctx, `SELECT jobs.mode,jobs.status,links.site_user_id,jobs.after_epoch
		FROM gateway_sync_jobs jobs JOIN gateway_site_links links USING (athlete_id,target)
		WHERE jobs.athlete_id=$1 AND jobs.target='rust'`, link.AthleteID).Scan(&mode, &status, &userID, &afterEpoch)
	if err != nil || mode != "initial" || status != "queued" || userID != 17 || len(fake.codes) != 1 || fake.codes[0] != "fixture-code" {
		t.Fatalf("initial sync = %q %q user %d; error = %v", mode, status, userID, err)
	}
	cutoff := time.Now().Add(-30 * 24 * time.Hour).Unix()
	if afterEpoch < cutoff-5 || afterEpoch > cutoff {
		t.Fatalf("initial sync after_epoch = %d, want within five seconds of %d", afterEpoch, cutoff)
	}
}
