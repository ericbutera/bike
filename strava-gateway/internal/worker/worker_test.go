package worker

import (
	"context"
	"errors"
	"net/http"
	"os"
	"testing"
	"time"

	"github.com/ericbutera/bike-services/strava-gateway/internal/provider"
	"github.com/ericbutera/bike-services/strava-gateway/internal/storage"
	"github.com/jackc/pgx/v5/pgxpool"
)

var errTransportFailure = errors.New("connection reset")

func TestClassifyActivityFailure(t *testing.T) {
	tests := []struct {
		name      string
		err       error
		refreshed bool
		want      activityFailureDisposition
	}{
		{name: "unauthorized refresh", err: provider.HTTPError{StatusCode: http.StatusUnauthorized}, want: activityFailureRefresh},
		{name: "unauthorized after refresh", err: provider.HTTPError{StatusCode: http.StatusUnauthorized}, refreshed: true, want: activityFailureRevoke},
		{name: "private activity", err: provider.HTTPError{StatusCode: http.StatusForbidden}, want: activityFailureDelete},
		{name: "deleted activity", err: provider.HTTPError{StatusCode: http.StatusNotFound}, want: activityFailureDelete},
		{name: "rate limit retries", err: provider.HTTPError{StatusCode: http.StatusTooManyRequests}, want: activityFailureRetry},
		{name: "server failure retries", err: provider.HTTPError{StatusCode: http.StatusServiceUnavailable}, want: activityFailureRetry},
		{name: "transport failure retries", err: errTransportFailure, want: activityFailureRetry},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if got := classifyActivityFailure(test.err, test.refreshed); got != test.want {
				t.Fatalf("disposition = %v, want %v", got, test.want)
			}
		})
	}
}

func TestQuotaWaitingReasonDistinguishesWindow(t *testing.T) {
	tests := []struct {
		buckets []string
		want    string
	}{
		{buckets: []string{"overall_15m", "nonupload_15m"}, want: "short_window_quota"},
		{buckets: []string{"overall_day", "nonupload_day"}, want: "daily_quota"},
		{buckets: []string{"overall_15m", "nonupload_day"}, want: "daily_and_short_window_quota"},
		{buckets: []string{"unknown_bucket"}, want: "rate_limit"},
		{buckets: nil, want: "rate_limit"},
	}
	for _, test := range tests {
		if got := quotaWaitingReason(test.buckets); got != test.want {
			t.Fatalf("quotaWaitingReason(%v) = %q, want %q", test.buckets, got, test.want)
		}
	}
}

func TestSyncWaitingReasonUsesExhaustedQuotaWindows(t *testing.T) {
	databaseURL := os.Getenv("TEST_DATABASE_URL")
	if databaseURL == "" {
		t.Skip("TEST_DATABASE_URL is not set")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Second)
	defer cancel()
	if err := storage.Migrate(ctx, databaseURL); err != nil {
		t.Fatal(err)
	}
	pool, err := pgxpool.New(ctx, databaseURL)
	if err != nil {
		t.Fatal(err)
	}
	defer pool.Close()
	defer func() {
		if _, err := pool.Exec(context.Background(), `DELETE FROM gateway_rate_limits`); err != nil {
			t.Errorf("clean fixture rate limits: %v", err)
		}
	}()

	for _, test := range []struct {
		name   string
		limits [4]int
		want   string
	}{
		{name: "short window", limits: [4]int{1, 100, 1, 100}, want: "short_window_quota"},
		{name: "daily", limits: [4]int{100, 1, 100, 1}, want: "daily_quota"},
	} {
		t.Run(test.name, func(t *testing.T) {
			if _, err := pool.Exec(ctx, `DELETE FROM gateway_rate_limits`); err != nil {
				t.Fatal(err)
			}
			quota := storage.Quota{DB: pool, Limits: test.limits}
			if retryAt, err := quota.Acquire(ctx); err != nil || !retryAt.IsZero() {
				t.Fatalf("initial quota reservation = %s, %v", retryAt, err)
			}
			retryAt, err := quota.Acquire(ctx)
			if err != nil || !retryAt.After(time.Now()) {
				t.Fatalf("exhausted quota retry time = %s, %v", retryAt, err)
			}
			worker := Worker{Quota: quota}
			got := worker.syncWaitingReason(ctx, provider.HTTPError{StatusCode: http.StatusTooManyRequests, RetryAt: retryAt})
			if got != test.want {
				t.Fatalf("sync waiting reason = %q, want %q", got, test.want)
			}
		})
	}
}

func TestNormalizeMissingStreams(t *testing.T) {
	response, err := normalizeMissingStreams(provider.Response{StatusCode: http.StatusNotFound}, provider.HTTPError{StatusCode: http.StatusNotFound})
	if err != nil {
		t.Fatalf("normalizing missing streams: %v", err)
	}
	if got, want := string(response.Body), `{}`; got != want {
		t.Fatalf("body = %q, want %q", got, want)
	}

	response, err = normalizeMissingStreams(provider.Response{}, provider.HTTPError{StatusCode: http.StatusServiceUnavailable})
	if err == nil || response.Body != nil {
		t.Fatalf("server failure should remain retryable: response=%+v error=%v", response, err)
	}
}
