package render

import (
	"context"
	"errors"
	"sync"
	"testing"
	"time"

	"github.com/ericbutera/bike/map-renderer/internal/observability"
	"github.com/stretchr/testify/require"
)

type fakeBrowser struct {
	mu      sync.Mutex
	calls   int
	err     error
	started chan struct{}
	unblock chan struct{}
}

func (b *fakeBrowser) Render(context.Context, Request) ([]byte, error) {
	b.mu.Lock()
	b.calls++
	b.mu.Unlock()
	if b.started != nil {
		b.started <- struct{}{}
		<-b.unblock
	}
	return []byte("png"), b.err
}

func newFixtureRenderer(t *testing.T, browser Browser) *Renderer {
	t.Helper()
	metrics := observability.NewMetrics()
	r := New(browser, metrics)
	t.Cleanup(func() {
		ctx, cancel := context.WithTimeout(context.Background(), time.Second)
		defer cancel()
		require.NoError(t, r.Close(ctx))
	})
	return r
}

func TestSnapshotsAreStatelessAndFailuresRecover(t *testing.T) {
	browser := &fakeBrowser{}
	r := newFixtureRenderer(t, browser)
	for range 2 {
		result, err := r.Render(context.Background(), fixtureRequest())
		require.NoError(t, err)
		require.Equal(t, []byte("png"), result.PNG)
	}
	require.Equal(t, 2, browser.calls)
	browser.err = errors.New("fixture browser failure")
	_, err := r.Render(context.Background(), fixtureRequest())
	require.ErrorIs(t, err, browser.err)
	browser.err = nil
	_, err = r.Render(context.Background(), fixtureRequest())
	require.NoError(t, err)
	_, err = r.Render(context.Background(), Request{})
	require.ErrorIs(t, err, ErrRequest)
}

func TestQueueSerializesRendering(t *testing.T) {
	browser := &fakeBrowser{started: make(chan struct{}, 2), unblock: make(chan struct{}, 2)}
	r := newFixtureRenderer(t, browser)
	finished := make(chan error, 2)
	go func() { _, err := r.Render(context.Background(), fixtureRequest()); finished <- err }()
	<-browser.started
	request := fixtureRequest()
	request.Theme = "dark"
	go func() { _, err := r.Render(context.Background(), request); finished <- err }()
	require.Eventually(t, func() bool {
		r.mu.Lock()
		defer r.mu.Unlock()
		return len(r.queue) == 1
	}, time.Second, time.Millisecond)
	browser.unblock <- struct{}{}
	<-browser.started
	browser.unblock <- struct{}{}
	require.NoError(t, <-finished)
	require.NoError(t, <-finished)
}

func TestCanceledCallerDoesNotAbandonAcceptedWork(t *testing.T) {
	browser := &fakeBrowser{started: make(chan struct{}, 1), unblock: make(chan struct{}, 1)}
	r := newFixtureRenderer(t, browser)
	ctx, cancel := context.WithCancel(context.Background())
	finished := make(chan error, 1)
	go func() { _, err := r.Render(ctx, fixtureRequest()); finished <- err }()
	<-browser.started
	cancel()
	require.ErrorIs(t, <-finished, context.Canceled)
	browser.unblock <- struct{}{}
	require.NoError(t, r.Close(context.Background()))
}
