package main

import (
	"context"
	"errors"
	"log"
	"log/slog"
	"net"
	"net/http"
	"os"
	"os/signal"
	"strconv"
	"strings"
	"syscall"
	"time"

	stravav1 "github.com/ericbutera/bike-services/strava-gateway/gen/bike/strava/v1"
	"github.com/ericbutera/bike-services/strava-gateway/internal/oauth"
	"github.com/ericbutera/bike-services/strava-gateway/internal/observability"
	"github.com/ericbutera/bike-services/strava-gateway/internal/provider"
	"github.com/ericbutera/bike-services/strava-gateway/internal/secret"
	"github.com/ericbutera/bike-services/strava-gateway/internal/storage"
	"github.com/ericbutera/bike-services/strava-gateway/internal/webhook"
	"github.com/exaring/otelpgx"
	"github.com/jackc/pgx/v5/pgxpool"
	"go.opentelemetry.io/contrib/instrumentation/google.golang.org/grpc/otelgrpc"
	"go.opentelemetry.io/contrib/instrumentation/net/http/otelhttp"
	"google.golang.org/grpc"
)

func main() {
	quotaLimits, err := storage.ParseQuotaLimits(os.LookupEnv)
	if err != nil {
		log.Fatal(err)
	}

	databaseURL := required("DATABASE_URL")
	cipher, err := secret.NewTokenCipher(required("TOKEN_ENCRYPTION_KEY"))
	if err != nil {
		log.Fatal(err)
	}
	subscriptionID, err := strconv.ParseInt(required("STRAVA_SUBSCRIPTION_ID"), 10, 64)
	if err != nil || subscriptionID <= 0 {
		log.Fatal("STRAVA_SUBSCRIPTION_ID must be a positive integer")
	}
	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGTERM, syscall.SIGINT)
	defer stop()
	slog.SetDefault(observability.NewLogger("bike-strava-gateway", os.Stdout))
	shutdownTracing, err := observability.InitTracing(ctx, "bike-strava-gateway")
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
	startupContext, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()
	poolConfig, err := pgxpool.ParseConfig(databaseURL)
	if err != nil {
		log.Fatal(err)
	}
	poolConfig.ConnConfig.Tracer = otelpgx.NewTracer()
	pool, err := pgxpool.NewWithConfig(startupContext, poolConfig)
	if err != nil {
		log.Fatal(err)
	}
	defer pool.Close()
	if err := pool.Ping(startupContext); err != nil {
		log.Fatal(err)
	}
	handler := webhook.Handler{
		Inbox:          storage.Inbox{DB: pool, Targets: []string{"rust"}, Metrics: metrics},
		Metrics:        metrics,
		VerifyToken:    required("STRAVA_WEBHOOK_VERIFY_TOKEN"),
		SubscriptionID: subscriptionID,
		SigningSecret:  os.Getenv("STRAVA_WEBHOOK_SIGNING_SECRET"),
		AllowUnsigned:  strings.EqualFold(os.Getenv("ALLOW_UNSIGNED_WEBHOOKS"), "true"),
	}
	if err := handler.Validate(); err != nil {
		log.Fatal(err)
	}
	mux := http.NewServeMux()
	mux.Handle("/webhooks/strava", handler.Routes())
	mux.HandleFunc("/webhooks/strava-gateway", handler.Webhook)
	oauthHandler := oauth.Handler{
		States:      storage.OAuthStates{DB: pool},
		Syncs:       storage.Syncs{DB: pool},
		Connections: storage.Connections{DB: pool, Cipher: cipher},
		Quota:       storage.Quota{DB: pool, Limits: quotaLimits},
		Provider:    provider.Client{ClientID: required("STRAVA_CLIENT_ID"), ClientSecret: required("STRAVA_CLIENT_SECRET"), Metrics: metrics},
		CallbackURL: required("STRAVA_CALLBACK_URL"),
		Sites: map[string]oauth.Site{
			"rust": {Secret: required("TARGET_RUST_SECRET"), ReturnURL: required("RUST_ACCOUNT_URL")},
		},
	}
	oauthHandler.Routes(mux)
	mux.HandleFunc("GET /oauth/gateway/callback", oauthHandler.Callback)
	mux.Handle("/metrics", metrics.Handler())
	grpcListener, err := net.Listen("tcp", ":50051")
	if err != nil {
		log.Fatal(err)
	}
	grpcServer := grpc.NewServer(
		grpc.StatsHandler(otelgrpc.NewServerHandler()),
		grpc.UnaryInterceptor(metrics.UnaryServerInterceptor),
	)
	stravav1.RegisterGatewayServiceServer(grpcServer, oauth.GRPCServer{Handler: oauthHandler})
	go func() {
		if err := grpcServer.Serve(grpcListener); err != nil {
			log.Printf("gateway gRPC server stopped: %v", err)
		}
	}()
	defer grpcServer.GracefulStop()
	mux.HandleFunc("/healthz", func(w http.ResponseWriter, _ *http.Request) { w.WriteHeader(http.StatusOK) })
	mux.HandleFunc("/readyz", func(w http.ResponseWriter, r *http.Request) {
		ctx, cancel := context.WithTimeout(r.Context(), time.Second)
		defer cancel()
		if err := pool.Ping(ctx); err != nil {
			http.Error(w, "database unavailable", http.StatusServiceUnavailable)
			return
		}
		w.WriteHeader(http.StatusOK)
	})
	addr := os.Getenv("GATEWAY_ADDR")
	if addr == "" {
		addr = ":8080"
	}
	server := &http.Server{
		Addr: addr, Handler: otelhttp.NewHandler(metrics.HTTPMiddleware(mux), "bike.strava.gateway.http",
			otelhttp.WithFilter(func(request *http.Request) bool {
				return request.URL.Path != "/metrics" && request.URL.Path != "/healthz" && request.URL.Path != "/readyz"
			})),
		ReadHeaderTimeout: 5 * time.Second, WriteTimeout: 20 * time.Second,
	}
	metricsAddr := os.Getenv("METRICS_ADDR")
	if metricsAddr == "" {
		metricsAddr = ":9090"
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
	log.Printf("gateway listening on %s", addr)
	serverErrors := make(chan error, 1)
	go func() { serverErrors <- server.ListenAndServe() }()
	select {
	case err := <-serverErrors:
		if err != nil && !errors.Is(err, http.ErrServerClosed) {
			log.Fatal(err)
		}
	case err := <-metricsServerErrors:
		if err != nil && !errors.Is(err, http.ErrServerClosed) {
			log.Fatalf("metrics server stopped: %v", err)
		}
	case <-ctx.Done():
		shutdownContext, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		if err := server.Shutdown(shutdownContext); err != nil {
			log.Printf("HTTP server shutdown failed: %v", err)
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
