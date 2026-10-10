package render

import (
	"context"
	"errors"
	"sync"
	"time"

	"github.com/ericbutera/bike/map-renderer/internal/observability"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/attribute"
	"go.opentelemetry.io/otel/trace"
)

var ErrStopped = errors.New("renderer is shutting down")

type Browser interface {
	Render(context.Context, Request) ([]byte, error)
}
type Result struct{ PNG []byte }
type outcome struct {
	result Result
	err    error
}
type queuedRequest struct {
	ctx      context.Context
	request  Request
	response chan outcome
	enqueued time.Time
}

// Renderer owns the serialized Chromium queue. Bike's API owns image caching.
type Renderer struct {
	browser Browser
	metrics *observability.Metrics
	mu      sync.Mutex
	ready   *sync.Cond
	queue   []queuedRequest
	stopped bool
	done    chan struct{}
}

func New(browser Browser, metrics *observability.Metrics) *Renderer {
	r := &Renderer{browser: browser, metrics: metrics, done: make(chan struct{})}
	r.ready = sync.NewCond(&r.mu)
	go r.processQueue()
	return r
}

func (r *Renderer) Render(ctx context.Context, request Request) (Result, error) {
	if err := request.Validate(); err != nil {
		return Result{}, err
	}
	job := queuedRequest{ctx: context.WithoutCancel(ctx), request: request,
		response: make(chan outcome, 1), enqueued: time.Now()}
	r.mu.Lock()
	if r.stopped {
		r.mu.Unlock()
		return Result{}, ErrStopped
	}
	r.queue = append(r.queue, job)
	r.metrics.QueueDepth.Inc()
	r.ready.Signal()
	r.mu.Unlock()
	select {
	case outcome := <-job.response:
		return outcome.result, outcome.err
	case <-ctx.Done():
		return Result{}, ctx.Err()
	}
}

func (r *Renderer) next() (queuedRequest, bool) {
	r.mu.Lock()
	defer r.mu.Unlock()
	for len(r.queue) == 0 && !r.stopped {
		r.ready.Wait()
	}
	if len(r.queue) == 0 {
		return queuedRequest{}, false
	}
	job := r.queue[0]
	r.queue[0] = queuedRequest{}
	r.queue = r.queue[1:]
	return job, true
}

func (r *Renderer) processQueue() {
	defer close(r.done)
	for {
		job, ok := r.next()
		if !ok {
			return
		}
		r.metrics.QueueDepth.Dec()
		r.metrics.InFlight.Inc()
		wait := time.Since(job.enqueued).Seconds()
		r.metrics.QueueWait.Observe(wait)
		ctx, span := otel.Tracer("bike-map-renderer").Start(job.ctx, "bike.maps.render_queue_wait",
			trace.WithAttributes(attribute.Float64("queue.wait_seconds", wait)))
		result, err := r.render(ctx, job.request)
		span.End()
		r.metrics.InFlight.Dec()
		job.response <- outcome{result, err}
	}
}

func (r *Renderer) Close(ctx context.Context) error {
	r.mu.Lock()
	r.stopped = true
	r.ready.Broadcast()
	r.mu.Unlock()
	select {
	case <-r.done:
		return nil
	case <-ctx.Done():
		return ctx.Err()
	}
}

func (r *Renderer) render(ctx context.Context, request Request) (Result, error) {
	ctx, span := otel.Tracer("bike-map-renderer").Start(ctx, "bike.maps.render",
		trace.WithAttributes(attribute.String("map.variant", request.Variant), attribute.String("map.theme", request.Theme)))
	defer span.End()
	started := time.Now()
	outcome := "error"
	defer func() { r.metrics.RenderDuration.WithLabelValues(outcome).Observe(time.Since(started).Seconds()) }()
	image, err := r.browser.Render(ctx, request)
	if err != nil {
		r.metrics.Failures.WithLabelValues("browser_render").Inc()
		observability.Error(ctx, "Map rendering failed", err, "browser_render")
		return Result{}, err
	}
	r.metrics.ImagesGenerated.Inc()
	outcome = "success"
	return Result{PNG: image}, nil
}
