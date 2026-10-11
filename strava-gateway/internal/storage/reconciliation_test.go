package storage

import (
	"context"
	"encoding/json"
	"errors"
	"strings"
	"testing"

	"github.com/ericbutera/bike/strava-gateway/internal/webhook"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgconn"
)

type reconciliationTransaction struct {
	pgx.Tx
	comparisonError error
	comparisons     int
	committed       bool
	rolledBack      bool
	processed       bool
	payload         []byte
}

func (tx *reconciliationTransaction) Begin(context.Context) (pgx.Tx, error) { return tx, nil }
func (tx *reconciliationTransaction) Commit(context.Context) error {
	tx.committed = true
	return nil
}
func (tx *reconciliationTransaction) Rollback(context.Context) error {
	tx.rolledBack = !tx.committed
	return pgx.ErrTxClosed
}
func (tx *reconciliationTransaction) Exec(_ context.Context, statement string, _ ...any) (pgconn.CommandTag, error) {
	if strings.HasPrefix(statement, "UPDATE strava_delivery_outbox AS current") {
		tx.comparisons++
		if tx.comparisonError != nil {
			return pgconn.CommandTag{}, tx.comparisonError
		}
	}
	if strings.HasPrefix(statement, "UPDATE strava_webhook_events SET status='processed'") {
		tx.processed = true
	}
	return pgconn.NewCommandTag("UPDATE 1"), nil
}
func (tx *reconciliationTransaction) QueryRow(_ context.Context, _ string, arguments ...any) pgx.Row {
	tx.payload = arguments[4].([]byte)
	return receiptRow(func(destinations ...any) error {
		*destinations[0].(*int64) = 1
		return nil
	})
}

func TestReconciliationCompletionScopeAndAtomicFailure(t *testing.T) {
	failure := errors.New("comparison unavailable")
	cases := []struct {
		name        string
		event       webhook.Event
		operation   string
		failure     error
		comparisons int
	}{
		{name: "scheduled", event: webhook.Event{Reconciliation: true}, operation: "upsert", comparisons: 1},
		{name: "webhook", event: webhook.Event{SubscriptionID: 12, Reconciliation: true}, operation: "upsert"},
		{name: "manual", operation: "upsert"},
		{name: "delete", event: webhook.Event{Reconciliation: true}, operation: "delete"},
		{name: "comparison failure", event: webhook.Event{Reconciliation: true}, operation: "upsert", failure: failure, comparisons: 1},
	}
	for _, item := range cases {
		t.Run(item.name, func(t *testing.T) {
			tx := &reconciliationTransaction{comparisonError: item.failure}
			var artifact *Artifact
			if item.operation == "upsert" {
				artifact = &Artifact{SHA256: strings.Repeat("a", 64), RelativePath: "fixture.json", SizeBytes: 123}
			}
			err := (Jobs{DB: tx}).CompleteEvent(context.Background(), EventJob{ID: 1, Event: item.event}, item.operation, artifact)
			if !errors.Is(err, item.failure) || tx.comparisons != item.comparisons {
				t.Fatalf("completion error=%v comparisons=%d; want error=%v comparisons=%d", err, tx.comparisons, item.failure, item.comparisons)
			}
			if item.failure != nil {
				if tx.committed || tx.processed || !tx.rolledBack {
					t.Fatal("comparison failure must roll back without acknowledging the event")
				}
			} else if !tx.committed || !tx.processed || tx.rolledBack {
				t.Fatal("successful completion must commit the event and delivery changes together")
			}
		})
	}
}

func TestSyncPageRetainsScheduledOriginAndManualResync(t *testing.T) {
	for _, scheduled := range []bool{false, true} {
		tx := &reconciliationTransaction{}
		job := SyncJob{ID: 1, AthleteID: 7, Target: "rust", Reconciliation: scheduled}
		if err := (Syncs{DB: tx}).CompletePage(context.Background(), job, []int64{42}); err != nil {
			t.Fatal(err)
		}
		var event webhook.Event
		if err := json.Unmarshal(tx.payload, &event); err != nil {
			t.Fatal(err)
		}
		if event.Reconciliation != scheduled || event.SubscriptionID != 0 || event.OwnerID != 7 || event.ObjectID != 42 || !tx.committed {
			t.Fatalf("persisted sync event lost origin or ownership: %+v", event)
		}
	}
}
