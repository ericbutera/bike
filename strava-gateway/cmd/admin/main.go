package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"log"
	"os"
	"strconv"
	"time"

	"github.com/ericbutera/bike/strava-gateway/internal/storage"
	"github.com/ericbutera/bike/strava-gateway/internal/worker"
	"github.com/jackc/pgx/v5/pgxpool"
)

func main() {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	if err := run(ctx, os.Args[1:]); err != nil {
		log.Fatal(err)
	}
}

func run(ctx context.Context, args []string) error {
	if len(args) == 0 {
		return errors.New("usage: admin status | admin failures | admin artifact <sha256> | admin replay <event|delivery|sync> <id>")
	}
	if args[0] == "artifact" {
		if len(args) != 2 {
			return errors.New("usage: admin artifact <sha256>")
		}
		root := os.Getenv("ARTIFACTS_DIR")
		if root == "" {
			return errors.New("ARTIFACTS_DIR is required; run in the worker pod")
		}
		body, err := (worker.Artifacts{Root: root}).ReadByHash(args[1])
		if err != nil {
			return err
		}
		_, err = os.Stdout.Write(body)
		return err
	}
	databaseURL := os.Getenv("DATABASE_URL")
	if databaseURL == "" {
		return errors.New("DATABASE_URL is required")
	}
	pool, err := pgxpool.New(ctx, databaseURL)
	if err != nil {
		return err
	}
	defer pool.Close()
	operations := storage.Operations{DB: pool}
	switch args[0] {
	case "status":
		if len(args) != 1 {
			return errors.New("usage: admin status")
		}
		counts, err := operations.Counts(ctx)
		if err != nil {
			return err
		}
		for _, item := range counts {
			fmt.Printf("%s %s %d\n", item.Kind, item.Status, item.Count)
		}
		return nil
	case "replay":
		if len(args) != 3 {
			return errors.New("usage: admin replay <event|delivery|sync> <id>")
		}
		id, err := strconv.ParseInt(args[2], 10, 64)
		if err != nil {
			return err
		}
		if err := operations.ReplayDead(ctx, args[1], id); err != nil {
			return err
		}
		fmt.Printf("replayed %s %d\n", args[1], id)
		return nil
	case "failures":
		if len(args) != 1 {
			return errors.New("usage: admin failures")
		}
		jobs, err := operations.DeadJobs(ctx, 50)
		if err != nil {
			return err
		}
		return json.NewEncoder(os.Stdout).Encode(jobs)
	default:
		return errors.New("usage: admin status | admin failures | admin artifact <sha256> | admin replay <event|delivery|sync> <id>")
	}
}
