package storage

import (
	"context"
	"database/sql"
	"embed"
	"errors"
	"io/fs"

	_ "github.com/jackc/pgx/v5/stdlib"
	"github.com/pressly/goose/v3"
)

//go:embed migrations/*.sql
var migrations embed.FS

// Migrate runs append-only migrations. Production invokes this through a Job
// before the HTTP deployment starts; serving requests never changes schema.
func Migrate(ctx context.Context, databaseURL string) (err error) {
	db, err := sql.Open("pgx", databaseURL)
	if err != nil {
		return err
	}
	defer func() { err = errors.Join(err, db.Close()) }()
	migrationFiles, err := fs.Sub(migrations, "migrations")
	if err != nil {
		return err
	}
	provider, err := goose.NewProvider(goose.DialectPostgres, db, migrationFiles,
		goose.WithTableName("strava_gateway_schema_versions"))
	if err != nil {
		return err
	}
	_, err = provider.Up(ctx)
	return err
}
