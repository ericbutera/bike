package provider

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"
	"time"
)

type fixtureTransport func(*http.Request) (*http.Response, error)

func (transport fixtureTransport) RoundTrip(request *http.Request) (*http.Response, error) {
	return transport(request)
}

func fixtureBody(t *testing.T, name string) []byte {
	t.Helper()
	body, err := os.ReadFile("testdata/" + name)
	if err != nil {
		t.Fatal(err)
	}
	return body
}

func TestExchangeAndRefreshUseFixtureResponses(t *testing.T) {
	var grants []string
	httpClient := &http.Client{Transport: fixtureTransport(func(request *http.Request) (*http.Response, error) {
		if request.Method != http.MethodPost || request.URL.String() != "https://strava.test/api/v3/oauth/token" {
			t.Fatalf("unexpected token request: %s %s", request.Method, request.URL)
		}
		if request.Header.Get("Content-Type") != "application/x-www-form-urlencoded" {
			t.Fatal("token request must use form encoding")
		}
		if err := request.ParseForm(); err != nil {
			t.Fatal(err)
		}
		if request.Form.Get("client_id") != "fixture-client" || request.Form.Get("client_secret") != "fixture-secret" {
			t.Fatal("client credentials missing from token request")
		}
		grant := request.Form.Get("grant_type")
		grants = append(grants, grant)
		name := "token-exchange.json"
		switch grant {
		case "authorization_code":
			if request.Form.Get("code") != "fixture-code" {
				t.Fatal("authorization code missing from exchange")
			}
		case "refresh_token":
			name = "token-refresh.json"
			if request.Form.Get("refresh_token") != "fixture-refresh" {
				t.Fatal("refresh did not use the exchange token")
			}
		default:
			t.Fatalf("unexpected grant: %q", grant)
		}
		return &http.Response{StatusCode: http.StatusOK, Header: make(http.Header),
			Body: io.NopCloser(strings.NewReader(string(fixtureBody(t, name))))}, nil
	})}
	client := Client{HTTP: httpClient, AuthBaseURL: "https://strava.test",
		ClientID: "fixture-client", ClientSecret: "fixture-secret"}
	exchanged, _, err := client.ExchangeCode(context.Background(), "fixture-code")
	if err != nil || exchanged.Athlete.ID != 887654321 || exchanged.AccessToken != "fixture-access" ||
		!HasScope(ParseScopes(exchanged.Scope), "activity:read_all") {
		t.Fatalf("exchange fixture did not produce the expected athlete, scope, and tokens: %v", err)
	}
	refreshed, _, err := client.Refresh(context.Background(), exchanged.RefreshToken)
	if err != nil || refreshed.AccessToken != "fixture-access-rotated" ||
		refreshed.RefreshToken != "fixture-refresh-rotated" || refreshed.ExpiresAt <= exchanged.ExpiresAt {
		t.Fatalf("refresh fixture did not rotate tokens and extend expiry: %v", err)
	}
	if len(grants) != 2 {
		t.Fatalf("token request count = %d, want 2", len(grants))
	}
}

func TestAuthenticatedAthleteUsesFixtureResponse(t *testing.T) {
	client := Client{APIBaseURL: "https://strava.test/api/v3", HTTP: &http.Client{
		Transport: fixtureTransport(func(request *http.Request) (*http.Response, error) {
			if request.Method != http.MethodGet || request.URL.Path != "/api/v3/athlete" ||
				request.Header.Get("Authorization") != "Bearer fixture-access" {
				t.Fatalf("unexpected athlete request: %s %s", request.Method, request.URL)
			}
			return &http.Response{StatusCode: http.StatusOK, Header: make(http.Header),
				Body: io.NopCloser(strings.NewReader(string(fixtureBody(t, "athlete.json"))))}, nil
		}),
	}}
	athlete, _, err := client.AuthenticatedAthlete(context.Background(), "fixture-access")
	if err != nil || athlete != 887654321 {
		t.Fatalf("athlete = %d, error = %v", athlete, err)
	}
}

func TestSyncFetchesListDetailAndStreamsWithRealRideFixture(t *testing.T) {
	var payload struct {
		Activity json.RawMessage `json:"activity"`
		Streams  json.RawMessage `json:"streams"`
	}
	if err := json.Unmarshal(fixtureBody(t, "strava-real-ride.json"), &payload); err != nil {
		t.Fatal(err)
	}
	var requests []string
	client := Client{APIBaseURL: "https://strava.test/api/v3", HTTP: &http.Client{
		Transport: fixtureTransport(func(request *http.Request) (*http.Response, error) {
			if request.Method != http.MethodGet || request.Header.Get("Authorization") != "Bearer fixture-access" {
				t.Fatal("sync must use GET and the supplied bearer credential")
			}
			requests = append(requests, request.URL.Path)
			var body []byte
			switch request.URL.Path {
			case "/api/v3/athlete/activities":
				if request.URL.Query().Get("page") != "1" || request.URL.Query().Get("after") != "1790856000" {
					t.Fatalf("sync query = %s", request.URL.RawQuery)
				}
				body = append(append([]byte{'['}, payload.Activity...), ']')
			case "/api/v3/activities/887654322":
				body = payload.Activity
			case "/api/v3/activities/887654322/streams":
				if request.URL.Query().Get("key_by_type") != "true" || !strings.Contains(request.URL.Query().Get("keys"), "latlng") {
					t.Fatal("sync must request route streams keyed by type")
				}
				body = payload.Streams
			default:
				t.Fatalf("unexpected provider request: %s", request.URL)
			}
			return &http.Response{StatusCode: http.StatusOK, Header: make(http.Header),
				Body: io.NopCloser(bytes.NewReader(body))}, nil
		}),
	}}
	ctx := context.Background()
	ids, _, err := client.ListActivities(ctx, "fixture-access", 1, 1790856000)
	if err != nil || len(ids) != 1 || ids[0] != 887654322 {
		t.Fatalf("activity list = %v, error = %v", ids, err)
	}
	detail, err := client.FetchActivity(ctx, "fixture-access", ids[0])
	if err != nil || !bytes.Equal(detail.Body, payload.Activity) {
		t.Fatalf("detail fixture did not survive the adapter: %v", err)
	}
	activity, err := ParseActivity(detail.Body)
	if err != nil || !activity.Cycling() {
		t.Fatalf("fixture is not recognized as a cycling activity: %v", err)
	}
	streams, err := client.FetchStreams(ctx, "fixture-access", ids[0])
	if err != nil || !bytes.Equal(streams.Body, payload.Streams) || len(requests) != 3 {
		t.Fatalf("stream fixture or request count mismatch: %v", err)
	}
}

func TestFetchActivityNormalizesHTTPFailures(t *testing.T) {
	tests := []struct {
		name       string
		status     int
		kind       FailureKind
		retryAfter string
	}{
		{name: "unauthorized", status: http.StatusUnauthorized, kind: FailureUnauthorized},
		{name: "private activity", status: http.StatusForbidden, kind: FailureForbidden},
		{name: "deleted activity", status: http.StatusNotFound, kind: FailureNotFound},
		{name: "rate limited", status: http.StatusTooManyRequests, kind: FailureRateLimited, retryAfter: "60"},
		{name: "server error", status: http.StatusServiceUnavailable, kind: FailureOther},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
				if test.retryAfter != "" {
					w.Header().Set("Retry-After", test.retryAfter)
				}
				w.WriteHeader(test.status)
			}))
			defer server.Close()

			client := Client{APIBaseURL: server.URL, HTTP: server.Client()}
			response, err := client.FetchActivity(context.Background(), "token", 42)
			var remote HTTPError
			if !errors.As(err, &remote) {
				t.Fatalf("expected HTTPError, got %v", err)
			}
			if response.StatusCode != test.status {
				t.Fatalf("status = %d, want %d", response.StatusCode, test.status)
			}
			if remote.Kind() != test.kind {
				t.Fatalf("failure kind = %v, want %v", remote.Kind(), test.kind)
			}
			if test.retryAfter != "" && time.Until(remote.RetryAt) < 50*time.Second {
				t.Fatalf("retry time = %s, want at least 50s in the future", remote.RetryAt)
			}
		})
	}
}

func TestRetryAfterHTTPDate(t *testing.T) {
	want := time.Now().Add(2 * time.Minute).UTC().Truncate(time.Second)
	got := retryAt(http.Header{"Retry-After": {want.Format(http.TimeFormat)}})
	if !got.Equal(want) {
		t.Fatalf("retry time = %s, want %s", got, want)
	}
}
