package storage

import (
	"context"
	"testing"
	"time"

	"github.com/jackc/pgx/v5"
)

type receiptRow func(...any) error

func (row receiptRow) Scan(destinations ...any) error { return row(destinations...) }

type receiptDatabase struct {
	pgx.Tx
	row pgx.Row
}

func (db receiptDatabase) QueryRow(context.Context, string, ...any) pgx.Row { return db.row }
func (db receiptDatabase) Begin(context.Context) (pgx.Tx, error)            { return db, nil }
func (db receiptDatabase) Commit(context.Context) error                     { return nil }
func (db receiptDatabase) Rollback(context.Context) error                   { return pgx.ErrTxClosed }

func TestWorkerDeliveryClaimRetainsReceiptFetchAndLatestClaimClocks(t *testing.T) {
	received := time.Date(2026, 10, 10, 12, 0, 0, 0, time.UTC)
	fetched := received.Add(time.Minute)
	claimed := received.Add(2 * time.Minute)
	db := receiptDatabase{row: receiptRow(func(destinations ...any) error {
		*destinations[0].(*int64) = 42
		*destinations[2].(*[]byte) = []byte(`{"owner_id":7,"object_id":8}`)
		*destinations[3].(*string) = "rust"
		*destinations[4].(*string) = "delete"
		*destinations[8].(*time.Time) = received.Add(30 * time.Second)
		*destinations[12].(*time.Time) = received
		*destinations[13].(*time.Time) = fetched
		*destinations[14].(*time.Time) = claimed
		return nil
	})}
	job, err := (Jobs{DB: db}).ClaimDelivery(context.Background())
	if err != nil || job == nil {
		t.Fatalf("claim delivery: %v %v", job, err)
	}
	if !job.ReceivedAt.Equal(received) || !job.FetchedAt.Equal(fetched) || !job.ClaimedAt.Equal(claimed) {
		t.Fatalf("distinct pipeline clocks were lost: %+v", job)
	}
	if job.ReceivedAt.Equal(job.CreatedAt) {
		t.Fatal("outbox acceptance replaced original receipt")
	}
}

func TestWorkerSyncClaimPreservesOriginalClockAndLegacyClockStaysUnknown(t *testing.T) {
	received := time.Date(2026, 10, 10, 12, 0, 0, 0, time.UTC)
	db := receiptDatabase{row: receiptRow(func(destinations ...any) error {
		*destinations[0].(*int64) = 42
		*destinations[10].(*time.Time) = received
		return nil
	})}
	job, err := (Syncs{DB: db}).Claim(context.Background())
	if err != nil || job == nil {
		t.Fatalf("claim sync: %v %v", job, err)
	}
	if receipt := syncReceipt(*job); receipt == nil || !receipt.Equal(received) {
		t.Fatalf("sync receipt: %v", receipt)
	}
	if syncReceipt(SyncJob{}) != nil {
		t.Fatal("legacy sync clock must stay unknown")
	}
}
