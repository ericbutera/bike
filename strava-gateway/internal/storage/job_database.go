package storage

import (
	"context"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgconn"
)

// JobDatabase is the pgx boundary used by durable claims and transactions.
// A pgxpool.Pool implements it; tests can exercise row mapping independently
// of the PostgreSQL lease and migration integration fixtures.
type JobDatabase interface {
	QueryRow(context.Context, string, ...any) pgx.Row
	Exec(context.Context, string, ...any) (pgconn.CommandTag, error)
	Begin(context.Context) (pgx.Tx, error)
}
