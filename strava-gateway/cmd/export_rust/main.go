package main

import (
	"context"
	"errors"
	"fmt"
	"log"
	"os"
	"time"

	"github.com/ericbutera/bike/strava-gateway/internal/secret"
	"github.com/ericbutera/bike/strava-gateway/internal/storage"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"
)

func main() {
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Minute)
	defer cancel()
	if err := run(ctx, os.Args[1:]); err != nil {
		log.Fatal(err)
	}
}

func run(ctx context.Context, args []string) (err error) {
	apply := false
	if len(args) == 1 && args[0] == "--apply" {
		apply = true
	} else if len(args) != 0 {
		return errors.New("usage: export-rust [--apply]")
	}
	if os.Getenv("RUST_DATABASE_URL") == "" || os.Getenv("DATABASE_URL") == "" {
		return errors.New("RUST_DATABASE_URL and DATABASE_URL are required")
	}
	cipher, err := secret.NewTokenCipher(os.Getenv("TOKEN_ENCRYPTION_KEY"))
	if err != nil {
		return err
	}
	rustDB, err := pgxpool.New(ctx, os.Getenv("RUST_DATABASE_URL"))
	if err != nil {
		return err
	}
	defer rustDB.Close()
	gatewayDB, err := pgxpool.New(ctx, os.Getenv("DATABASE_URL"))
	if err != nil {
		return err
	}
	defer gatewayDB.Close()
	connections := storage.Connections{DB: gatewayDB, Cipher: cipher}
	rows, err := rustDB.Query(ctx, `SELECT user_id,athlete_id FROM strava_connections ORDER BY id`)
	if err != nil {
		return err
	}
	type owner struct{ userID, athleteID int64 }
	var owners []owner
	for rows.Next() {
		var item owner
		if err := rows.Scan(&item.userID, &item.athleteID); err != nil {
			rows.Close()
			return err
		}
		owners = append(owners, item)
	}
	err = rows.Err()
	rows.Close()
	if err != nil {
		return err
	}
	if len(owners) == 0 {
		return errors.New("no Rust connections to restore")
	}
	type restoration struct {
		owner      owner
		connection storage.Connection
	}
	var ready []restoration
	for _, item := range owners {
		link, err := connections.FindLink(ctx, item.athleteID, "rust")
		if err != nil {
			return fmt.Errorf("rust athlete %d has no gateway link: %w", item.athleteID, err)
		}
		if link.UserID != item.userID {
			return fmt.Errorf("rust athlete %d gateway link belongs to another user", item.athleteID)
		}
		current, err := connections.Find(ctx, item.athleteID)
		if err != nil {
			return fmt.Errorf("read gateway athlete %d: %w", item.athleteID, err)
		}
		if current.AccessToken == "" || current.RefreshToken == "" {
			return fmt.Errorf("gateway athlete %d has no usable token pair", item.athleteID)
		}
		ready = append(ready, restoration{owner: item, connection: current})
	}
	if apply {
		var tx pgx.Tx
		tx, err = rustDB.Begin(ctx)
		if err != nil {
			return err
		}
		defer func() {
			cleanupCtx, cancel := context.WithTimeout(context.WithoutCancel(ctx), 5*time.Second)
			defer cancel()
			if cleanupErr := tx.Rollback(cleanupCtx); cleanupErr != nil && !errors.Is(cleanupErr, pgx.ErrTxClosed) {
				err = errors.Join(err, cleanupErr)
			}
		}()
		for _, item := range ready {
			result, err := tx.Exec(ctx, `UPDATE strava_connections SET access_token=$1,
			refresh_token=$2,expires_at=$3,updated_at=now()
			WHERE user_id=$4 AND athlete_id=$5`, item.connection.AccessToken,
				item.connection.RefreshToken, item.connection.ExpiresAt,
				item.owner.userID, item.owner.athleteID)
			if err != nil {
				return err
			}
			if result.RowsAffected() != 1 {
				return fmt.Errorf("rust athlete %d connection changed during export", item.owner.athleteID)
			}
		}
		if err := tx.Commit(ctx); err != nil {
			return err
		}
	}
	if apply {
		fmt.Printf("restored %d Rust token pairs from gateway\n", len(owners))
	} else {
		fmt.Printf("eligible Rust token pairs: %d; no changes made\n", len(owners))
	}
	return nil
}
