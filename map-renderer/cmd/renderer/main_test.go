package main

import (
	"context"
	"github.com/stretchr/testify/require"
	"google.golang.org/grpc"
	"google.golang.org/grpc/health"
	"google.golang.org/grpc/health/grpc_health_v1"
	"net"
	"testing"
)

func TestHealthcheckCommandAndInvalidConfiguration(t *testing.T) {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	require.NoError(t, err)
	server := grpc.NewServer()
	readiness := health.NewServer()
	readiness.SetServingStatus("bike.maps.v1.MapService", grpc_health_v1.HealthCheckResponse_SERVING)
	grpc_health_v1.RegisterHealthServer(server, readiness)
	finished := make(chan error, 1)
	go func() { finished <- server.Serve(listener) }()
	t.Cleanup(func() { server.GracefulStop(); require.NoError(t, <-finished) })
	t.Setenv("MAP_GRPC_ADDRESS", listener.Addr().String())
	require.NoError(t, run(context.Background(), []string{"healthcheck"}))
	readiness.SetServingStatus("bike.maps.v1.MapService", grpc_health_v1.HealthCheckResponse_NOT_SERVING)
	require.Error(t, run(context.Background(), []string{"healthcheck"}))
	require.Error(t, run(context.Background(), []string{"other"}))
	t.Setenv("METRICS_PORT", "invalid")
	require.Error(t, run(context.Background(), []string{"healthcheck"}))
}

func TestServingCommandReportsBrowserStartupFailure(t *testing.T) {
	t.Setenv("OTEL_TRACES_EXPORTER", "none")
	t.Setenv("CHROME_EXECUTABLE", "/nonexistent/bike-fixture-chromium")
	require.ErrorContains(t, run(context.Background(), nil), "bike-fixture-chromium")
}
