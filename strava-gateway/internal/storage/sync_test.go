package storage

import (
	"testing"
	"time"
)

func TestInitialAndIncrementalSyncUseThirtyDayWindow(t *testing.T) {
	now := time.Date(2026, 10, 6, 12, 0, 0, 0, time.UTC)
	for _, mode := range []string{"initial", "incremental"} {
		if got, want := syncAfterEpoch(mode, now), now.Add(-30*24*time.Hour).Unix(); got != want {
			t.Fatalf("%s sync cutoff = %d, want %d", mode, got, want)
		}
	}
}
