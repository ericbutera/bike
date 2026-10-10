package observability

import (
	"strconv"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/collectors"
	"github.com/prometheus/client_golang/prometheus/promauto"
)

var buckets = []float64{0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10, 30, 60}

type Metrics struct {
	Registry        *prometheus.Registry
	GRPCRequests    *prometheus.CounterVec
	GRPCDuration    *prometheus.HistogramVec
	ImagesGenerated prometheus.Counter
	Failures        *prometheus.CounterVec
	RenderDuration  *prometheus.HistogramVec
	QueueWait       prometheus.Histogram
	QueueDepth      prometheus.Gauge
	InFlight        prometheus.Gauge
}

func NewMetrics() *Metrics {
	registry := prometheus.NewRegistry()
	process := prometheus.WrapRegistererWithPrefix("bike_maps_process_", registry)
	process.MustRegister(collectors.NewProcessCollector(collectors.ProcessCollectorOpts{}), collectors.NewGoCollector())
	factory := promauto.With(registry)
	grpcLabels := []string{"method", "status_code"}
	return &Metrics{
		Registry:        registry,
		GRPCRequests:    factory.NewCounterVec(counter("grpc_requests_total", "Map renderer gRPC requests."), grpcLabels),
		GRPCDuration:    factory.NewHistogramVec(histogram("grpc_request_duration_seconds", "Map renderer gRPC request duration in seconds."), grpcLabels),
		ImagesGenerated: factory.NewCounter(counter("images_generated_total", "Successfully generated map snapshots.")),
		Failures:        factory.NewCounterVec(counter("render_failures_total", "Map rendering failures by bounded stage."), []string{"stage"}),
		RenderDuration:  factory.NewHistogramVec(histogram("render_duration_seconds", "Chromium snapshot pipeline duration."), []string{"outcome"}),
		QueueWait:       factory.NewHistogram(histogram("render_queue_wait_seconds", "Time map renders wait in the serialized browser queue.")),
		QueueDepth:      factory.NewGauge(gauge("render_queue_depth", "Map renders waiting for the browser queue.")),
		InFlight:        factory.NewGauge(gauge("renders_in_flight", "Map renders currently using the browser.")),
	}
}

func counter(name, help string) prometheus.CounterOpts {
	return prometheus.CounterOpts{Namespace: "bike_maps", Name: name, Help: help}
}
func gauge(name, help string) prometheus.GaugeOpts {
	return prometheus.GaugeOpts{Namespace: "bike_maps", Name: name, Help: help}
}
func histogram(name, help string) prometheus.HistogramOpts {
	return prometheus.HistogramOpts{Namespace: "bike_maps", Name: name, Help: help, Buckets: buckets}
}

func (m *Metrics) GRPC(status int, started time.Time) {
	labels := []string{"Render", strconv.Itoa(status)}
	m.GRPCRequests.WithLabelValues(labels...).Inc()
	m.GRPCDuration.WithLabelValues(labels...).Observe(time.Since(started).Seconds())
}
