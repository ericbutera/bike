package app

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"sync/atomic"
	"testing"
	"time"

	"github.com/ericbutera/bike/map-renderer/internal/render"
	"github.com/stretchr/testify/require"
)

type fixtureBrowser struct{ closed atomic.Bool }

func (*fixtureBrowser) Render(context.Context, render.Request) ([]byte, error) {
	return []byte("fixture-png"), nil
}
func (b *fixtureBrowser) Close() error { b.closed.Store(true); return nil }

func freeAddress(t *testing.T) string {
	t.Helper()
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	require.NoError(t, err)
	address := listener.Addr().String()
	require.NoError(t, listener.Close())
	return address
}

func runtimeConfig(t *testing.T) Config {
	t.Helper()
	t.Setenv("OTEL_TRACES_EXPORTER", "none")
	return Config{AssetsAddress: freeAddress(t), GRPCAddress: freeAddress(t), MetricsPort: 9090,
		Assets: t.TempDir()}
}

func TestRuntimeStartsGRPCAndMetricsAndDrainsBeforeBrowserCloses(t *testing.T) {
	config := runtimeConfig(t)
	metrics := freeAddress(t)
	_, port, err := net.SplitHostPort(metrics)
	require.NoError(t, err)
	_, err = fmt.Sscan(port, &config.MetricsPort)
	require.NoError(t, err)
	browser := &fixtureBrowser{}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	finished := make(chan error, 1)
	go func() {
		finished <- run(ctx, config, func(context.Context, Config) (browserRuntime, error) { return browser, nil })
	}()
	require.Eventually(t, func() bool { return CheckHealth(context.Background(), config) == nil }, 3*time.Second, 10*time.Millisecond)
	response, err := http.Get("http://" + config.AssetsAddress + "/render")
	require.NoError(t, err)
	require.NoError(t, response.Body.Close())
	require.Equal(t, 404, response.StatusCode)
	response, err = http.Get("http://" + metrics + "/metrics")
	require.NoError(t, err)
	data, err := io.ReadAll(response.Body)
	require.NoError(t, err)
	require.NoError(t, response.Body.Close())
	require.Contains(t, string(data), "bike_maps_render_queue_depth")
	cancel()
	require.NoError(t, <-finished)
	require.True(t, browser.closed.Load())
}

func TestRuntimeStartupFailuresReleaseResources(t *testing.T) {
	failure := errors.New("browser startup fixture")
	config := runtimeConfig(t)
	err := run(context.Background(), config, func(context.Context, Config) (browserRuntime, error) { return nil, failure })
	require.ErrorIs(t, err, failure)
	listener, err := net.Listen("tcp", config.AssetsAddress)
	require.NoError(t, err)
	t.Cleanup(func() { require.NoError(t, listener.Close()) })
	browser := &fixtureBrowser{}
	err = run(context.Background(), config, func(context.Context, Config) (browserRuntime, error) { return browser, nil })
	require.Error(t, err)
	require.True(t, browser.closed.Load())
	_, err = openBrowser(context.Background(), Config{AssetsAddress: "invalid"})
	require.Error(t, err)
	t.Setenv("OTEL_SDK_DISABLED", "invalid")
	err = run(context.Background(), config, func(context.Context, Config) (browserRuntime, error) { return browser, nil })
	require.Error(t, err)
}

func TestHealthcheckRejectsUnhealthyAndUnreachableServices(t *testing.T) {
	config := Config{GRPCAddress: freeAddress(t)}
	require.Error(t, CheckHealth(context.Background(), config))
}
