package main

import (
	"context"
	"errors"
	"log/slog"
	"os"
	"os/signal"
	"syscall"

	"github.com/ericbutera/bike/map-renderer/internal/app"
)

func main() {
	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGTERM, syscall.SIGINT)
	defer stop()
	if err := run(ctx, os.Args[1:]); err != nil {
		slog.Error("Map renderer failed", "error", err)
		os.Exit(1)
	}
}

func run(ctx context.Context, args []string) error {
	config, err := app.LoadConfig()
	if err != nil {
		return err
	}
	if len(args) == 1 && args[0] == "healthcheck" {
		return app.CheckHealth(ctx, config)
	}
	if len(args) != 0 {
		return errors.New("unknown renderer command")
	}
	return app.Run(ctx, config)
}
