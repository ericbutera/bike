package main

import (
	"context"
	"errors"
	"log"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/ericbutera/bike/strava-gateway/internal/observability"
	"github.com/ericbutera/bike/strava-gateway/internal/provider"
	"github.com/ericbutera/bike/strava-gateway/internal/secret"
	"github.com/ericbutera/bike/strava-gateway/internal/storage"
	"github.com/ericbutera/bike/strava-gateway/internal/worker"
	"github.com/exaring/otelpgx"
	"github.com/jackc/pgx/v5/pgxpool"
	"go.opentelemetry.io/contrib/instrumentation/net/http/otelhttp"
)

func main() {
	quotaLimits, err := storage.ParseQuotaLimits(os.LookupEnv)
	if err != nil {
		log.Fatal(err)
	}

	cipher, err := secret.NewTokenCipher(required("TOKEN_ENCRYPTION_KEY"))
	if err != nil {
		log.Fatal(err)
	}
	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGTERM, syscall.SIGINT)
	defer stop()
	slog.SetDefault(observability.NewLogger("bike-strava-worker", os.Stdout))
	shutdownTracing, err := observability.InitTracing(ctx, "bike-strava-worker")
	if err != nil {
		log.Fatal(err)
	}
	defer func() {
		shutdownContext, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		if err := shutdownTracing(shutdownContext); err != nil {
			log.Printf("trace exporter shutdown failed: %v", err)
		}
	}()
	metrics := observability.NewMetrics()
	http.DefaultTransport = otelhttp.NewTransport(http.DefaultTransport)
	poolConfig, err := pgxpool.ParseConfig(required("DATABASE_URL"))
	if err != nil {
		log.Fatal(err)
	}
	poolConfig.ConnConfig.Tracer = otelpgx.NewTracer()
	pool, err := pgxpool.NewWithConfig(ctx, poolConfig)
	if err != nil {
		log.Fatal(err)
	}
	defer pool.Close()
	if err := pool.Ping(ctx); err != nil {
		log.Fatal(err)
	}
	service := worker.Worker{
		Jobs:        storage.Jobs{DB: pool},
		Syncs:       storage.Syncs{DB: pool},
		Connections: storage.Connections{DB: pool, Cipher: cipher},
		Quota:       storage.Quota{DB: pool, Limits: quotaLimits},
		Provider: provider.Client{
			ClientID: required("STRAVA_CLIENT_ID"), ClientSecret: required("STRAVA_CLIENT_SECRET"), Metrics: metrics,
		},
		Artifacts: worker.Artifacts{Root: required("ARTIFACTS_DIR")},
		Sender: worker.DeliverySender{Targets: map[string]worker.Target{
			"rust": {URL: required("TARGET_RUST_URL"), Secret: required("TARGET_RUST_SECRET")},
		}},
		Logger:  slog.Default(),
		Metrics: metrics,
	}
	metricsAddr := os.Getenv("METRICS_ADDR")
	if metricsAddr == "" {
		metricsAddr = ":9091"
	}
	metricsServer := &http.Server{Addr: metricsAddr, Handler: metrics.Handler(), ReadHeaderTimeout: 5 * time.Second}
	metricsServerErrors := make(chan error, 1)
	go func() { metricsServerErrors <- metricsServer.ListenAndServe() }()
	defer func() {
		shutdownContext, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		if err := metricsServer.Shutdown(shutdownContext); err != nil {
			log.Printf("metrics server shutdown failed: %v", err)
		}
	}()
	for ctx.Err() == nil {
		select {
		case err := <-metricsServerErrors:
			if !errors.Is(err, http.ErrServerClosed) {
				log.Printf("metrics server stopped: %v", err)
				stop()
			}
		default:
		}
		if err := service.Run(ctx); err != nil {
			slog.Error("gateway worker stopped", "error", err)
			select {
			case <-ctx.Done():
			case <-time.After(5 * time.Second):
			}
		}
	}
}

func required(key string) string {
	value := os.Getenv(key)
	if value == "" {
		log.Fatalf("%s is required", key)
	}
	return value
}
