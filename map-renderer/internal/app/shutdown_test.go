package app

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	mapsv1 "github.com/ericbutera/bike/map-renderer/gen/bike/maps/v1"
	"github.com/ericbutera/bike/map-renderer/internal/observability"
	"github.com/ericbutera/bike/map-renderer/internal/render"
	"github.com/stretchr/testify/require"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/status"
)

type assetBrowser struct {
	url     string
	started chan struct{}
	release chan struct{}
	closed  atomic.Bool
	loaded  atomic.Bool
	once    sync.Once
}

func (b *assetBrowser) Render(ctx context.Context, _ render.Request) (png []byte, err error) {
	close(b.started)
	select {
	case <-b.release:
	case <-ctx.Done():
		return nil, ctx.Err()
	}
	if b.closed.Load() {
		return nil, fmt.Errorf("browser closed before accepted render finished")
	}
	client := &http.Client{Timeout: time.Second}
	response, err := client.Get(b.url)
	if err != nil {
		return nil, err
	}
	defer func() { err = errors.Join(err, response.Body.Close()) }()
	if response.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("asset server returned %d", response.StatusCode)
	}
	png, err = io.ReadAll(response.Body)
	b.loaded.Store(err == nil)
	return png, err
}

func (b *assetBrowser) Close() error { b.closed.Store(true); return nil }

func (b *assetBrowser) unblock() { b.once.Do(func() { close(b.release) }) }

func shutdownConfig(t *testing.T) Config {
	t.Helper()
	config := runtimeConfig(t)
	_, err := fmt.Sscanf(freeAddress(t), "127.0.0.1:%d", &config.MetricsPort)
	require.NoError(t, err)
	require.NoError(t, os.WriteFile(filepath.Join(config.Assets, "index.html"), []byte("snapshot-fixture"), 0600))
	return config
}

func startAssetRuntime(t *testing.T, config Config, browser *assetBrowser) (context.CancelFunc, <-chan error) {
	t.Helper()
	ctx, cancel := context.WithCancel(context.Background())
	finished := make(chan error, 1)
	stopped := make(chan struct{})
	go func() {
		defer close(stopped)
		finished <- run(ctx, config, func(context.Context, Config) (browserRuntime, error) { return browser, nil })
	}()
	t.Cleanup(func() {
		cancel()
		browser.unblock()
		select {
		case <-stopped:
		case <-time.After(2 * time.Second):
			t.Error("runtime did not release its resources")
		}
	})
	require.Eventually(t, func() bool { return CheckHealth(context.Background(), config) == nil }, 3*time.Second, 10*time.Millisecond)
	return cancel, finished
}

func startSnapshotRPC(t *testing.T, config Config, ctx context.Context) <-chan error {
	t.Helper()
	connection, err := grpc.NewClient(config.GRPCAddress, grpc.WithTransportCredentials(insecure.NewCredentials()))
	require.NoError(t, err)
	t.Cleanup(func() { require.NoError(t, connection.Close()) })
	rpcDone := make(chan error, 1)
	go func() {
		response, err := mapsv1.NewMapServiceClient(connection).Render(ctx, &mapsv1.RenderMapRequest{
			Theme: "light", Variant: "thumbnail", Dpr: 1,
			Points: []*mapsv1.RoutePoint{{Latitude: 40, Longitude: -80}, {Latitude: 41, Longitude: -81}},
		})
		if err == nil && string(response.Png) != "snapshot-fixture" {
			err = fmt.Errorf("render did not load its asset")
		}
		rpcDone <- err
	}()
	return rpcDone
}

func waitForBrowser(t *testing.T, browser *assetBrowser) {
	t.Helper()
	select {
	case <-browser.started:
	case <-time.After(3 * time.Second):
		t.Fatal("render never started")
	}
}

func TestShutdownKeepsAssetsAndBrowserAvailableForAcceptedRender(t *testing.T) {
	config := shutdownConfig(t)
	browser := &assetBrowser{url: "http://" + config.AssetsAddress + "/", started: make(chan struct{}), release: make(chan struct{})}
	cancel, finished := startAssetRuntime(t, config, browser)
	rpcCtx, cancelRPC := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancelRPC()
	rpcDone := startSnapshotRPC(t, config, rpcCtx)
	waitForBrowser(t, browser)
	cancel()
	require.Eventually(t, func() bool { return CheckHealth(context.Background(), config) != nil }, 3*time.Second, 10*time.Millisecond)
	require.False(t, browser.closed.Load())
	browser.unblock()
	require.NoError(t, <-rpcDone)
	require.NoError(t, <-finished)
	require.True(t, browser.closed.Load())
}

func TestShutdownDrainsAcceptedBrowserWorkAfterItsRPCCancels(t *testing.T) {
	config := shutdownConfig(t)
	browser := &assetBrowser{url: "http://" + config.AssetsAddress + "/", started: make(chan struct{}), release: make(chan struct{})}
	cancel, finished := startAssetRuntime(t, config, browser)
	rpcCtx, cancelRPC := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancelRPC()
	rpcDone := startSnapshotRPC(t, config, rpcCtx)
	waitForBrowser(t, browser)
	cancelRPC()
	require.Equal(t, codes.Canceled, status.Code(<-rpcDone))
	cancel()
	require.Eventually(t, func() bool { return CheckHealth(context.Background(), config) != nil }, 3*time.Second, 10*time.Millisecond)
	require.False(t, browser.closed.Load())
	browser.unblock()
	require.NoError(t, <-finished)
	require.True(t, browser.loaded.Load(), "accepted work must load its asset even after its caller canceled")
	require.True(t, browser.closed.Load())
}

func TestShutdownDeadlineStopsRPCAndClosesAssetListeners(t *testing.T) {
	config := shutdownConfig(t)
	browser := &assetBrowser{url: "http://" + config.AssetsAddress + "/", started: make(chan struct{}), release: make(chan struct{})}
	metrics := observability.NewMetrics()
	renderer := render.New(browser, metrics)
	servers := newServers(config, renderer, metrics)
	require.NoError(t, servers.start())
	t.Cleanup(func() {
		browser.unblock()
		cleanup, cancel := context.WithTimeout(context.Background(), time.Second)
		defer cancel()
		require.NoError(t, servers.shutdown(cleanup, renderer))
		require.NoError(t, browser.Close())
	})
	rpcCtx, cancelRPC := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancelRPC()
	rpcDone := startSnapshotRPC(t, config, rpcCtx)
	waitForBrowser(t, browser)
	shutdown, cancel := context.WithTimeout(context.Background(), 10*time.Millisecond)
	defer cancel()
	require.ErrorIs(t, servers.shutdown(shutdown, renderer), context.DeadlineExceeded)
	require.Error(t, <-rpcDone)
	client := &http.Client{Timeout: time.Second}
	response, err := client.Get(browser.url)
	if err == nil {
		require.NoError(t, response.Body.Close())
	}
	require.Error(t, err, "asset listener must close when the shutdown budget expires")
}
