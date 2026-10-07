package main

import (
	"context"
	"errors"
	"fmt"
	"log"
	"os"
	"slices"
	"strings"
	"time"

	"github.com/ericbutera/bike/strava-gateway/internal/secret"
	"github.com/ericbutera/bike/strava-gateway/internal/storage"
	"github.com/jackc/pgx/v5/pgxpool"
)

type sourceConnection struct {
	UserID     int64
	Connection storage.Connection
}

func main() {
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Minute)
	defer cancel()
	if err := run(ctx, os.Args[1:]); err != nil {
		log.Fatal(err)
	}
}

func run(ctx context.Context, args []string) error {
	apply := false
	replace := false
	if len(args) == 1 && args[0] == "--apply" {
		apply = true
	} else if len(args) == 2 && args[0] == "--apply" && args[1] == "--replace-tokens" {
		apply = true
		replace = true
	} else if len(args) != 0 {
		return errors.New("usage: import-rust [--apply [--replace-tokens]]")
	}
	rustURL := os.Getenv("RUST_DATABASE_URL")
	if rustURL == "" {
		return errors.New("RUST_DATABASE_URL is required")
	}
	rustDB, err := pgxpool.New(ctx, rustURL)
	if err != nil {
		return err
	}
	defer rustDB.Close()
	connections, err := readRustConnections(ctx, rustDB)
	if err != nil {
		return err
	}
	if !apply {
		fmt.Printf("eligible Rust connections: %d; no changes made\n", len(connections))
		return nil
	}
	if len(connections) == 0 {
		return errors.New("no Rust connections to import")
	}
	gatewayURL := os.Getenv("DATABASE_URL")
	if gatewayURL == "" {
		return errors.New("DATABASE_URL is required")
	}
	cipher, err := secret.NewTokenCipher(os.Getenv("TOKEN_ENCRYPTION_KEY"))
	if err != nil {
		return err
	}
	gatewayDB, err := pgxpool.New(ctx, gatewayURL)
	if err != nil {
		return err
	}
	defer gatewayDB.Close()
	store := storage.Connections{DB: gatewayDB, Cipher: cipher}
	var written int
	for _, item := range connections {
		created, err := store.ImportRust(ctx, item.Connection, item.UserID, replace)
		if err != nil {
			return fmt.Errorf("import Rust athlete %d: %w", item.Connection.AthleteID, err)
		}
		if created {
			written++
		}
	}
	fmt.Printf("linked %d Rust connections; wrote %d encrypted token pairs; replacement mode: %t\n",
		len(connections), written, replace)
	return nil
}

func readRustConnections(ctx context.Context, db *pgxpool.Pool) ([]sourceConnection, error) {
	rows, err := db.Query(ctx, `SELECT user_id,athlete_id,scopes,access_token,
		refresh_token,expires_at FROM strava_connections
		WHERE access_token<>'' AND refresh_token<>'' ORDER BY id`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	result := []sourceConnection{}
	for rows.Next() {
		var item sourceConnection
		var scopes string
		if err := rows.Scan(&item.UserID, &item.Connection.AthleteID, &scopes,
			&item.Connection.AccessToken, &item.Connection.RefreshToken,
			&item.Connection.ExpiresAt); err != nil {
			return nil, err
		}
		item.Connection.Scopes = strings.FieldsFunc(scopes, func(character rune) bool {
			return character == ',' || character == ' '
		})
		if !slices.Contains(item.Connection.Scopes, "activity:read_all") {
			return nil, fmt.Errorf("rust athlete %d lacks activity:read_all scope", item.Connection.AthleteID)
		}
		result = append(result, item)
	}
	return result, rows.Err()
}
