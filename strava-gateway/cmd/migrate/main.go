package main

import (
	"context"
	"log"
	"os"
	"time"

	"github.com/ericbutera/bike/strava-gateway/internal/storage"
)

func main() {
	databaseURL := os.Getenv("DATABASE_URL")
	if databaseURL == "" {
		log.Fatal("DATABASE_URL is required")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Minute)
	defer cancel()
	if err := storage.Migrate(ctx, databaseURL); err != nil {
		log.Fatal(err)
	}
}
