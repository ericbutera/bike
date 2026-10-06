package storage

import (
	"context"
	"errors"
	"time"

	"github.com/jackc/pgx/v5"
)

// Rollback is a no-op after commit; retain unexpected cleanup failures.
func rollbackTransaction(ctx context.Context, tx pgx.Tx, result *error) {
	cleanupCtx, cancel := context.WithTimeout(context.WithoutCancel(ctx), 5*time.Second)
	defer cancel()
	if err := tx.Rollback(cleanupCtx); err != nil && !errors.Is(err, pgx.ErrTxClosed) {
		*result = errors.Join(*result, err)
	}
}
