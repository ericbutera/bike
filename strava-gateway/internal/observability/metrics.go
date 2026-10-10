package observability

import (
	"context"
	"log/slog"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/collectors"
	"github.com/prometheus/client_golang/prometheus/promhttp"
	"go.opentelemetry.io/otel/trace"
	"google.golang.org/grpc"
	"google.golang.org/grpc/status"
)

type Metrics struct {
	registry           *prometheus.Registry
	httpRequests       *prometheus.CounterVec
	httpDuration       *prometheus.HistogramVec
	grpcRequests       *prometheus.CounterVec
	grpcDuration       *prometheus.HistogramVec
	providerRequests   *prometheus.CounterVec
	providerDuration   *prometheus.HistogramVec
	webhookEvents      *prometheus.CounterVec
	workerJobs         *prometheus.CounterVec
	workerDuration     *prometheus.HistogramVec
	deliveryAttempts   *prometheus.CounterVec
	deliveryDuration   *prometheus.HistogramVec
	deliveriesEnqueued *prometheus.CounterVec
	deliveryQueueAge   *prometheus.HistogramVec
	artifactsCreated   prometheus.Counter
	artifactBytes      prometheus.Counter
	artifactFailures   *prometheus.CounterVec
	artifactDiskUsage  prometheus.Gauge
	webhookQueueDepth  prometheus.Gauge
	deliveryQueueDepth prometheus.Gauge
	syncQueueDepth     prometheus.Gauge
	oldestWebhookAge   prometheus.Gauge
	oldestDeliveryAge  prometheus.Gauge
	oldestSyncAge      prometheus.Gauge
	stuckJobs          *prometheus.GaugeVec
	deadLetterJobs     *prometheus.GaugeVec
	quotaLimit         *prometheus.GaugeVec
	quotaUsed          *prometheus.GaugeVec
	quotaRemaining     *prometheus.GaugeVec
	quotaResetAt       *prometheus.GaugeVec
	quotaPauses        *prometheus.CounterVec
}

func NewMetrics() *Metrics {
	registry := prometheus.NewRegistry()
	prometheus.WrapRegistererWithPrefix("bike_strava_gateway_", registry).MustRegister(
		collectors.NewGoCollector(), collectors.NewProcessCollector(collectors.ProcessCollectorOpts{}),
	)
	metrics := &Metrics{
		registry:           registry,
		httpRequests:       prometheus.NewCounterVec(prometheus.CounterOpts{Name: "bike_strava_gateway_http_requests_total", Help: "Gateway HTTP requests."}, []string{"method", "route", "status_code"}),
		httpDuration:       prometheus.NewHistogramVec(prometheus.HistogramOpts{Name: "bike_strava_gateway_http_request_duration_seconds", Help: "Gateway HTTP request duration in seconds.", Buckets: requestBuckets()}, []string{"method", "route", "status_code"}),
		grpcRequests:       prometheus.NewCounterVec(prometheus.CounterOpts{Name: "bike_strava_gateway_grpc_requests_total", Help: "Gateway gRPC requests."}, []string{"method", "status_code"}),
		grpcDuration:       prometheus.NewHistogramVec(prometheus.HistogramOpts{Name: "bike_strava_gateway_grpc_request_duration_seconds", Help: "Gateway gRPC request duration in seconds.", Buckets: requestBuckets()}, []string{"method", "status_code"}),
		providerRequests:   prometheus.NewCounterVec(prometheus.CounterOpts{Name: "bike_strava_gateway_provider_requests_total", Help: "Outbound Strava API requests."}, []string{"operation", "status_code"}),
		providerDuration:   prometheus.NewHistogramVec(prometheus.HistogramOpts{Name: "bike_strava_gateway_provider_request_duration_seconds", Help: "Outbound Strava API request duration in seconds.", Buckets: requestBuckets()}, []string{"operation", "status_code"}),
		webhookEvents:      prometheus.NewCounterVec(prometheus.CounterOpts{Name: "bike_strava_gateway_webhook_events_total", Help: "Strava webhook callbacks by bounded outcome."}, []string{"outcome"}),
		workerJobs:         prometheus.NewCounterVec(prometheus.CounterOpts{Name: "bike_strava_gateway_worker_jobs_total", Help: "Gateway worker job attempts by type and outcome."}, []string{"job_type", "outcome"}),
		workerDuration:     prometheus.NewHistogramVec(prometheus.HistogramOpts{Name: "bike_strava_gateway_worker_job_duration_seconds", Help: "Gateway worker job duration in seconds.", Buckets: jobBuckets()}, []string{"job_type", "outcome"}),
		deliveryAttempts:   prometheus.NewCounterVec(prometheus.CounterOpts{Name: "bike_strava_gateway_delivery_attempts_total", Help: "Site delivery attempts by site and outcome."}, []string{"site", "outcome"}),
		deliveryDuration:   prometheus.NewHistogramVec(prometheus.HistogramOpts{Name: "bike_strava_gateway_delivery_duration_seconds", Help: "Site delivery duration in seconds.", Buckets: requestBuckets()}, []string{"site", "outcome"}),
		deliveriesEnqueued: prometheus.NewCounterVec(prometheus.CounterOpts{Name: "bike_strava_gateway_deliveries_enqueued_total", Help: "Site delivery jobs committed to the durable outbox."}, []string{"site"}),
		deliveryQueueAge:   prometheus.NewHistogramVec(prometheus.HistogramOpts{Name: "bike_strava_gateway_delivery_queue_age_seconds", Help: "Time from site delivery enqueue to its first processing attempt in seconds.", Buckets: jobBuckets()}, []string{"site"}),
		artifactsCreated:   prometheus.NewCounter(prometheus.CounterOpts{Name: "bike_strava_gateway_artifacts_created_total", Help: "Strava payload artifacts written to durable storage."}),
		artifactBytes:      prometheus.NewCounter(prometheus.CounterOpts{Name: "bike_strava_gateway_artifact_bytes_total", Help: "Bytes written as Strava payload artifacts."}),
		artifactFailures:   prometheus.NewCounterVec(prometheus.CounterOpts{Name: "bike_strava_gateway_artifact_failures_total", Help: "Strava artifact read/write failures."}, []string{"stage"}),
		artifactDiskUsage:  prometheus.NewGauge(prometheus.GaugeOpts{Name: "bike_strava_gateway_artifact_disk_usage_bytes", Help: "Current bytes used by files in the gateway artifact directory."}),
		webhookQueueDepth:  prometheus.NewGauge(prometheus.GaugeOpts{Name: "bike_strava_gateway_webhook_queue_depth", Help: "Pending webhook events awaiting processing."}),
		deliveryQueueDepth: prometheus.NewGauge(prometheus.GaugeOpts{Name: "bike_strava_gateway_delivery_queue_depth", Help: "Pending site deliveries awaiting processing."}),
		syncQueueDepth:     prometheus.NewGauge(prometheus.GaugeOpts{Name: "bike_strava_gateway_sync_queue_depth", Help: "Pending Strava sync pages awaiting processing."}),
		oldestWebhookAge:   prometheus.NewGauge(prometheus.GaugeOpts{Name: "bike_strava_gateway_webhook_oldest_age_seconds", Help: "Age of the oldest pending webhook event."}),
		oldestDeliveryAge:  prometheus.NewGauge(prometheus.GaugeOpts{Name: "bike_strava_gateway_delivery_oldest_age_seconds", Help: "Age of the oldest pending site delivery."}),
		oldestSyncAge:      prometheus.NewGauge(prometheus.GaugeOpts{Name: "bike_strava_gateway_sync_oldest_age_seconds", Help: "Age of the oldest queued Strava sync job."}),
		stuckJobs:          prometheus.NewGaugeVec(prometheus.GaugeOpts{Name: "bike_strava_gateway_stuck_jobs", Help: "Processing jobs whose lease has expired by queue."}, []string{"queue"}),
		deadLetterJobs:     prometheus.NewGaugeVec(prometheus.GaugeOpts{Name: "bike_strava_gateway_dead_letter_jobs", Help: "Unresolved dead jobs retained in the database by queue."}, []string{"queue"}),
		quotaLimit:         prometheus.NewGaugeVec(prometheus.GaugeOpts{Name: "bike_strava_gateway_quota_limit", Help: "Current shared Strava quota limit by bucket."}, []string{"bucket"}),
		quotaUsed:          prometheus.NewGaugeVec(prometheus.GaugeOpts{Name: "bike_strava_gateway_quota_used", Help: "Current shared Strava quota usage by bucket."}, []string{"bucket"}),
		quotaRemaining:     prometheus.NewGaugeVec(prometheus.GaugeOpts{Name: "bike_strava_gateway_quota_remaining", Help: "Current shared Strava quota remaining by bucket."}, []string{"bucket"}),
		quotaResetAt:       prometheus.NewGaugeVec(prometheus.GaugeOpts{Name: "bike_strava_gateway_quota_reset_timestamp_seconds", Help: "Unix timestamp when a shared Strava quota bucket resets."}, []string{"bucket"}),
		quotaPauses:        prometheus.NewCounterVec(prometheus.CounterOpts{Name: "bike_strava_gateway_quota_pauses_total", Help: "Times gateway work paused for provider quota availability."}, []string{"bucket"}),
	}
	registry.MustRegister(
		metrics.httpRequests, metrics.httpDuration, metrics.grpcRequests, metrics.grpcDuration,
		metrics.providerRequests, metrics.providerDuration, metrics.webhookEvents,
		metrics.workerJobs, metrics.workerDuration, metrics.deliveryAttempts, metrics.deliveryDuration,
		metrics.deliveriesEnqueued, metrics.deliveryQueueAge,
		metrics.artifactsCreated, metrics.artifactBytes, metrics.artifactFailures, metrics.artifactDiskUsage,
		metrics.webhookQueueDepth, metrics.deliveryQueueDepth, metrics.syncQueueDepth,
		metrics.oldestWebhookAge, metrics.oldestDeliveryAge, metrics.oldestSyncAge, metrics.stuckJobs, metrics.deadLetterJobs,
		metrics.quotaLimit, metrics.quotaUsed, metrics.quotaRemaining, metrics.quotaResetAt, metrics.quotaPauses,
	)
	metrics.SetDeadLetterStats([3]int64{})
	return metrics
}

func (metrics *Metrics) Handler() http.Handler {
	return promhttp.HandlerFor(metrics.registry, promhttp.HandlerOpts{})
}

func (metrics *Metrics) HTTPMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		if request.URL.Path == "/metrics" || request.URL.Path == "/healthz" || request.URL.Path == "/readyz" {
			next.ServeHTTP(writer, request)
			return
		}
		started := time.Now()
		recorder := &statusRecorder{ResponseWriter: writer}
		next.ServeHTTP(recorder, request)
		if recorder.status == 0 {
			recorder.status = http.StatusOK
		}
		route := request.Pattern
		if route == "" {
			route = "/unmatched"
		}
		duration := time.Since(started)
		metrics.RecordHTTP(request.Method, route, recorder.status, duration)
		if recorder.status >= http.StatusBadRequest || duration >= time.Second {
			logger := slog.Default()
			spanContext := trace.SpanFromContext(request.Context()).SpanContext()
			if spanContext.IsValid() {
				logger = logger.With("trace_id", spanContext.TraceID().String(), "span_id", spanContext.SpanID().String())
			}
			fields := []any{
				"method", boundedMethod(request.Method),
				"route", route,
				"status_code", recorder.status,
				"duration_ms", duration.Milliseconds(),
			}
			if recorder.status >= http.StatusInternalServerError {
				logger.Error("gateway HTTP request failed", fields...)
			} else if recorder.status >= http.StatusBadRequest {
				logger.Warn("gateway HTTP request rejected", fields...)
			} else {
				logger.Warn("slow gateway HTTP request", fields...)
			}
		}
	})
}

func (metrics *Metrics) UnaryServerInterceptor(ctx context.Context, request any, info *grpc.UnaryServerInfo, handler grpc.UnaryHandler) (any, error) {
	started := time.Now()
	response, err := handler(ctx, request)
	code := status.Code(err).String()
	metrics.grpcRequests.WithLabelValues(info.FullMethod, code).Inc()
	metrics.grpcDuration.WithLabelValues(info.FullMethod, code).Observe(time.Since(started).Seconds())
	return response, err
}

func (metrics *Metrics) RecordHTTP(method, route string, statusCode int, duration time.Duration) {
	method = boundedMethod(method)
	if route == "" {
		route = "/unmatched"
	}
	labels := []string{method, route, strconv.Itoa(statusCode)}
	metrics.httpRequests.WithLabelValues(labels...).Inc()
	metrics.httpDuration.WithLabelValues(labels...).Observe(duration.Seconds())
}

func (metrics *Metrics) RecordProviderRequest(path string, statusCode int, duration time.Duration) {
	operation := providerOperation(path)
	labels := []string{operation, strconv.Itoa(statusCode)}
	metrics.providerRequests.WithLabelValues(labels...).Inc()
	metrics.providerDuration.WithLabelValues(labels...).Observe(duration.Seconds())
}

func (metrics *Metrics) RecordQuota(headers http.Header) {
	setQuotaPair(metrics, "overall_15_minute", headers.Get("X-RateLimit-Limit"), headers.Get("X-RateLimit-Usage"), 0)
	setQuotaPair(metrics, "overall_daily", headers.Get("X-RateLimit-Limit"), headers.Get("X-RateLimit-Usage"), 1)
	setQuotaPair(metrics, "read_15_minute", headers.Get("X-ReadRateLimit-Limit"), headers.Get("X-ReadRateLimit-Usage"), 0)
	setQuotaPair(metrics, "read_daily", headers.Get("X-ReadRateLimit-Limit"), headers.Get("X-ReadRateLimit-Usage"), 1)
}

func setQuotaPair(metrics *Metrics, bucket, limits, usages string, index int) {
	limitValues := strings.Split(limits, ",")
	usageValues := strings.Split(usages, ",")
	if len(limitValues) <= index || len(usageValues) <= index {
		return
	}
	limit, limitErr := strconv.ParseFloat(strings.TrimSpace(limitValues[index]), 64)
	used, usedErr := strconv.ParseFloat(strings.TrimSpace(usageValues[index]), 64)
	if limitErr != nil || usedErr != nil || limit < 0 || used < 0 {
		return
	}
	metrics.quotaLimit.WithLabelValues(bucket).Set(limit)
	metrics.quotaUsed.WithLabelValues(bucket).Set(used)
	metrics.quotaRemaining.WithLabelValues(bucket).Set(max(0, limit-used))
}

func (metrics *Metrics) RecordWebhookOutcome(outcome string) {
	metrics.webhookEvents.WithLabelValues(bound(outcome, "accepted", "duplicate", "rejected", "invalid", "unavailable", "other")).Inc()
}

func (metrics *Metrics) RecordWorkerJob(jobType, outcome string, duration time.Duration) {
	jobType = bound(jobType, "webhook_event", "site_delivery", "strava_sync", "other")
	outcome = bound(outcome, "success", "failure", "retry", "ignored", "other")
	metrics.workerJobs.WithLabelValues(jobType, outcome).Inc()
	metrics.workerDuration.WithLabelValues(jobType, outcome).Observe(duration.Seconds())
}

func (metrics *Metrics) RecordDelivery(site, outcome string, duration time.Duration) {
	site = bound(site, "rust", "other")
	outcome = bound(outcome, "success", "failure", "retry", "other")
	metrics.deliveryAttempts.WithLabelValues(site, outcome).Inc()
	metrics.deliveryDuration.WithLabelValues(site, outcome).Observe(duration.Seconds())
}

func (metrics *Metrics) RecordDeliveryEnqueued(site string, count int64) {
	if count <= 0 {
		return
	}
	site = bound(site, "rust", "other")
	metrics.deliveriesEnqueued.WithLabelValues(site).Add(float64(count))
}

func (metrics *Metrics) RecordDeliveryQueueAge(site string, age time.Duration) {
	site = bound(site, "rust", "other")
	metrics.deliveryQueueAge.WithLabelValues(site).Observe(max(0, age.Seconds()))
}

func (metrics *Metrics) RecordQuotaPause(bucket string) {
	metrics.quotaPauses.WithLabelValues(quotaMetricBucket(bucket)).Inc()
}

func (metrics *Metrics) SetQuotaSnapshot(bucket string, limit, used int, resetAt time.Time) {
	bucket = quotaMetricBucket(bucket)
	metrics.quotaLimit.WithLabelValues(bucket).Set(float64(max(0, limit)))
	metrics.quotaUsed.WithLabelValues(bucket).Set(float64(max(0, used)))
	metrics.quotaRemaining.WithLabelValues(bucket).Set(float64(max(0, limit-used)))
	metrics.quotaResetAt.WithLabelValues(bucket).Set(float64(resetAt.Unix()))
}

func quotaMetricBucket(bucket string) string {
	switch bucket {
	case "nonupload_15m", "read_15_minute":
		return "read_15_minute"
	case "nonupload_day", "read_daily":
		return "read_daily"
	case "overall_15m", "overall_15_minute":
		return "overall_15_minute"
	case "overall_day", "overall_daily":
		return "overall_daily"
	default:
		return "other"
	}
}

func (metrics *Metrics) RecordArtifact(size int64) {
	metrics.artifactsCreated.Inc()
	metrics.artifactBytes.Add(float64(max(0, size)))
}

func (metrics *Metrics) RecordArtifactFailure(stage string) {
	metrics.artifactFailures.WithLabelValues(bound(stage, "read", "write", "other")).Inc()
}

func (metrics *Metrics) SetArtifactDiskUsage(bytes int64) {
	metrics.artifactDiskUsage.Set(float64(max(0, bytes)))
}

func (metrics *Metrics) SetQueueStats(webhooks, deliveries, syncs int64, oldestWebhook, oldestDelivery, oldestSync time.Duration, stuck [3]int64) {
	metrics.webhookQueueDepth.Set(float64(max(0, webhooks)))
	metrics.deliveryQueueDepth.Set(float64(max(0, deliveries)))
	metrics.syncQueueDepth.Set(float64(max(0, syncs)))
	metrics.oldestWebhookAge.Set(max(0, oldestWebhook.Seconds()))
	metrics.oldestDeliveryAge.Set(max(0, oldestDelivery.Seconds()))
	metrics.oldestSyncAge.Set(max(0, oldestSync.Seconds()))
	for index, queue := range []string{"webhook", "delivery", "sync"} {
		metrics.stuckJobs.WithLabelValues(queue).Set(float64(max(0, stuck[index])))
	}
}

func (metrics *Metrics) SetDeadLetterStats(counts [3]int64) {
	for index, queue := range []string{"webhook", "delivery", "sync"} {
		metrics.deadLetterJobs.WithLabelValues(queue).Set(float64(max(0, counts[index])))
	}
}

type statusRecorder struct {
	http.ResponseWriter
	status int
}

func (recorder *statusRecorder) WriteHeader(statusCode int) {
	if recorder.status != 0 {
		return
	}
	recorder.status = statusCode
	recorder.ResponseWriter.WriteHeader(statusCode)
}

func (recorder *statusRecorder) Write(body []byte) (int, error) {
	if recorder.status == 0 {
		recorder.WriteHeader(http.StatusOK)
	}
	return recorder.ResponseWriter.Write(body)
}

func (recorder *statusRecorder) Unwrap() http.ResponseWriter { return recorder.ResponseWriter }

func providerOperation(path string) string {
	switch {
	case strings.HasSuffix(path, "/oauth/token"):
		return "oauth_token"
	case path == "/athlete":
		return "athlete_profile"
	case path == "/athlete/activities":
		return "activities_list"
	case strings.HasPrefix(path, "/activities/") && strings.HasSuffix(path, "/streams"):
		return "activity_streams"
	case strings.HasPrefix(path, "/activities/"):
		return "activity_detail"
	default:
		return "other"
	}
}

func boundedMethod(method string) string {
	switch method {
	case http.MethodGet, http.MethodPost, http.MethodPut, http.MethodPatch, http.MethodDelete:
		return method
	default:
		return "OTHER"
	}
}

func bound(value string, allowed ...string) string {
	for _, candidate := range allowed {
		if value == candidate {
			return value
		}
	}
	return "other"
}

func requestBuckets() []float64 {
	return []float64{0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10, 30, 60}
}

func jobBuckets() []float64 {
	return []float64{0.01, 0.1, 1, 5, 10, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 21600, 86400}
}
