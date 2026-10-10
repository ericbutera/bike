package worker

import (
	"context"
	"github.com/ericbutera/bike/strava-gateway/internal/storage"
	"testing"
	"time"
)

func TestDeliveryRetriesKeepOriginalReceiptAndRun(t *testing.T) {
	received := time.Date(2026, 10, 10, 12, 0, 0, 0, time.UTC)
	job := storage.DeliveryJob{ReceivedAt: received, FetchedAt: received.Add(time.Minute), CreatedAt: received.Add(30 * time.Second), ClaimedAt: received.Add(2 * time.Minute), Attempts: 1}
	first := pipelineOrigin(context.Background(), "rust:42", job)
	job.Attempts = 2
	job.ClaimedAt = received.Add(time.Hour)
	second := pipelineOrigin(context.Background(), "rust:42", job)
	if first.RunID != second.RunID || len(first.RunID) != 36 || !second.StartedAt.Equal(received) {
		t.Fatal("Retry replaced the receipt or run")
	}
	if second.GatewayHistory[1].Attempts != 2 || !second.GatewayHistory[1].FinishedAt.Equal(job.ClaimedAt) {
		t.Fatal("Delivery evidence did not reflect the retry")
	}
	if second.Entrypoint != "strava_sync" {
		t.Fatal("Sync origin mislabeled")
	}
	job.Event.SubscriptionID = 9
	if pipelineOrigin(context.Background(), "rust:42", job).Entrypoint != "strava_webhook" {
		t.Fatal("Webhook origin mislabeled")
	}
	job.ReceivedAt = time.Time{}
	if pipelineOrigin(context.Background(), "rust:42", job) != nil {
		t.Fatal("Legacy receipt must stay unknown")
	}
}
