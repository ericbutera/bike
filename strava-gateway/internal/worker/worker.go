package worker

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"log/slog"
	"math/rand"
	"net/http"
	"strings"
	"time"

	"github.com/ericbutera/bike-services/strava-gateway/internal/observability"
	"github.com/ericbutera/bike-services/strava-gateway/internal/provider"
	"github.com/ericbutera/bike-services/strava-gateway/internal/storage"
	"go.opentelemetry.io/otel/attribute"
	"go.opentelemetry.io/otel/codes"
)

type Worker struct {
	Jobs        storage.Jobs
	Syncs       storage.Syncs
	Connections storage.Connections
	Quota       storage.Quota
	Provider    provider.SyncAPI
	Artifacts   Artifacts
	Sender      DeliverySender
	Logger      *slog.Logger
	Metrics     *observability.Metrics
}

type terminalSyncError struct{ err error }

func (err terminalSyncError) Error() string { return err.err.Error() }

func (err terminalSyncError) Unwrap() error { return err.err }

type activityFailureDisposition uint8

const (
	activityFailureRetry activityFailureDisposition = iota
	activityFailureRefresh
	activityFailureDelete
	activityFailureRevoke
)

func classifyActivityFailure(err error, refreshed bool) activityFailureDisposition {
	var remote provider.HTTPError
	if !errors.As(err, &remote) {
		return activityFailureRetry
	}
	switch remote.Kind() {
	case provider.FailureUnauthorized:
		if refreshed {
			return activityFailureRevoke
		}
		return activityFailureRefresh
	case provider.FailureForbidden, provider.FailureNotFound:
		return activityFailureDelete
	default:
		return activityFailureRetry
	}
}

func normalizeMissingStreams(response provider.Response, err error) (provider.Response, error) {
	if isStatus(err, http.StatusNotFound) {
		response.Body = []byte(`{}`)
		return response, nil
	}
	return response, err
}

func (worker Worker) Run(ctx context.Context) error {
	var lastReconciliation time.Time
	var lastMetrics time.Time
	for ctx.Err() == nil {
		if worker.Metrics != nil && time.Since(lastMetrics) >= 15*time.Second {
			worker.refreshMetrics(ctx)
			lastMetrics = time.Now()
		}
		if worker.Syncs.DB != nil && time.Since(lastReconciliation) >= time.Minute {
			count, err := worker.Syncs.QueueDueReconciliation(ctx, time.Now())
			if err != nil {
				return err
			}
			if count > 0 {
				worker.log().Info("queued Strava reconciliation", "site_links", count)
			}
			lastReconciliation = time.Now()
		}
		didWork, err := worker.Tick(ctx)
		if err != nil {
			return err
		}
		if didWork {
			continue
		}
		timer := time.NewTimer(2 * time.Second)
		select {
		case <-ctx.Done():
			timer.Stop()
			return nil
		case <-timer.C:
		}
	}
	return nil
}

func (worker Worker) Tick(ctx context.Context) (bool, error) {
	event, err := worker.Jobs.ClaimEvent(ctx)
	if err != nil {
		return false, err
	}
	if event != nil {
		jobContext, span := observability.StartJobSpan(ctx, event.TraceParent, event.TraceState,
			"bike.strava.worker.webhook_event", attribute.Int("job.attempt", event.Attempts))
		jobLogger := observability.WithSpanFields(worker.log(), span).With(
			"queue", "webhook", "job_id", event.ID, "athlete_id", event.Event.OwnerID,
			"activity_id", event.Event.ObjectID, "attempt", event.Attempts)
		started := time.Now()
		if err := worker.processEvent(jobContext, *event); err != nil {
			span.RecordError(err)
			span.SetStatus(codes.Error, "webhook event failed")
			var invalid invalidPayloadError
			permanent := errors.As(err, &invalid)
			if retryErr := worker.Jobs.RetryEvent(ctx, *event, retryTime(event.Attempts, err), err.Error(), permanent); retryErr != nil {
				worker.recordJob("webhook_event", "failure", time.Since(started))
				span.End()
				return true, retryErr
			}
			outcome := "retry"
			if permanent || event.Attempts >= 20 {
				outcome = "failure"
			}
			worker.logFailure(jobLogger, outcome, err)
			worker.recordJob("webhook_event", outcome, time.Since(started))
		} else {
			worker.recordJob("webhook_event", "success", time.Since(started))
		}
		span.End()
		return true, nil
	}
	delivery, err := worker.Jobs.ClaimDelivery(ctx)
	if err != nil {
		return false, err
	}
	if delivery == nil {
		if worker.Syncs.DB == nil {
			return false, nil
		}
		syncJob, err := worker.Syncs.Claim(ctx)
		if err != nil || syncJob == nil {
			return false, err
		}
		jobContext, span := observability.StartJobSpan(ctx, syncJob.TraceParent, syncJob.TraceState,
			"bike.strava.worker.sync", attribute.String("job.target", syncJob.Target), attribute.Int("job.attempt", syncJob.Attempts))
		jobLogger := observability.WithSpanFields(worker.log(), span).With(
			"queue", "sync", "job_id", syncJob.ID, "athlete_id", syncJob.AthleteID,
			"target", syncJob.Target, "page", syncJob.Page, "attempt", syncJob.Attempts)
		started := time.Now()
		if err := worker.processSync(jobContext, *syncJob); err != nil {
			span.RecordError(err)
			span.SetStatus(codes.Error, "Strava sync failed")
			var terminal terminalSyncError
			var invalid invalidPayloadError
			if errors.As(err, &invalid) {
				if failureErr := worker.Syncs.Fail(ctx, *syncJob, err.Error()); failureErr != nil {
					span.End()
					return true, failureErr
				}
				err = terminalSyncError{err: err}
			}
			if errors.As(err, &terminal) {
				worker.logFailure(jobLogger, "failure", err)
				worker.recordJob("strava_sync", "failure", time.Since(started))
				span.End()
			} else {
				if retryErr := worker.Syncs.Retry(ctx, *syncJob, retryTime(syncJob.Attempts, err), err.Error(), worker.syncWaitingReason(ctx, err)); retryErr != nil {
					worker.recordJob("strava_sync", "failure", time.Since(started))
					span.End()
					return true, retryErr
				}
				outcome := "retry"
				if syncJob.Attempts >= 20 {
					outcome = "failure"
				}
				worker.logFailure(jobLogger, outcome, err)
				worker.recordJob("strava_sync", outcome, time.Since(started))
			}
		} else {
			worker.recordJob("strava_sync", "success", time.Since(started))
		}
		span.End()
		return true, nil
	}
	jobContext, span := observability.StartJobSpan(ctx, delivery.TraceParent, delivery.TraceState,
		"bike.strava.worker.site_delivery", attribute.String("delivery.site", delivery.Target),
		attribute.String("delivery.operation", delivery.Operation), attribute.Int("job.attempt", delivery.Attempts))
	jobLogger := observability.WithSpanFields(worker.log(), span).With(
		"queue", "delivery", "job_id", delivery.ID, "event_id", delivery.EventID,
		"athlete_id", delivery.Event.OwnerID, "activity_id", delivery.Event.ObjectID,
		"target", delivery.Target, "attempt", delivery.Attempts)
	if delivery.Artifact != nil {
		jobLogger = jobLogger.With("artifact_sha256", delivery.Artifact.SHA256, "artifact_path", delivery.Artifact.RelativePath)
	}
	started := time.Now()
	if worker.Metrics != nil {
		worker.Metrics.RecordDeliveryQueueAge(delivery.Target, started.Sub(delivery.CreatedAt))
	}
	outcome := "success"
	if err := worker.processDelivery(jobContext, *delivery); err != nil {
		span.RecordError(err)
		span.SetStatus(codes.Error, "site delivery failed")
		if errors.Is(err, ErrArtifactUnavailable) {
			outcome = "retry"
			if worker.Metrics != nil {
				worker.Metrics.RecordArtifactFailure("read")
			}
			jobLogger.Warn("Strava artifact is unavailable; refetching", "event_id", delivery.EventID)
			if refetchErr := worker.Jobs.RequeueMissingArtifact(ctx, *delivery); refetchErr != nil {
				outcome = "failure"
				worker.recordDelivery(delivery.Target, outcome, time.Since(started))
				span.End()
				return true, refetchErr
			}
			worker.recordDelivery(delivery.Target, outcome, time.Since(started))
			span.End()
			return true, nil
		}
		var remoteErr DeliveryError
		permanent := errors.As(err, &remoteErr) && remoteErr.StatusCode >= 400 &&
			remoteErr.StatusCode < 500 && remoteErr.StatusCode != http.StatusTooManyRequests
		terminal := permanent || delivery.Attempts >= 20
		if retryErr := worker.Jobs.RetryDelivery(ctx, *delivery,
			retryTime(delivery.Attempts, err), err.Error(), permanent); retryErr != nil {
			outcome = "failure"
			worker.recordDelivery(delivery.Target, outcome, time.Since(started))
			span.End()
			return true, retryErr
		}
		if terminal {
			outcome = "failure"
		} else {
			outcome = "retry"
		}
		worker.logFailure(jobLogger, outcome, err)
	}
	worker.recordDelivery(delivery.Target, outcome, time.Since(started))
	span.End()
	return true, nil
}

func (worker Worker) syncWaitingReason(ctx context.Context, err error) string {
	var remote provider.HTTPError
	if !errors.As(err, &remote) || remote.Kind() != provider.FailureRateLimited {
		return ""
	}
	buckets, bucketErr := worker.Quota.BlockedBuckets(ctx)
	if bucketErr != nil {
		return "rate_limit"
	}
	return quotaWaitingReason(buckets)
}

func quotaWaitingReason(buckets []string) string {
	var daily, shortWindow bool
	for _, bucket := range buckets {
		switch {
		case strings.HasSuffix(bucket, "_day"):
			daily = true
		case strings.HasSuffix(bucket, "_15m"):
			shortWindow = true
		}
	}
	switch {
	case daily && shortWindow:
		return "daily_and_short_window_quota"
	case daily:
		return "daily_quota"
	case shortWindow:
		return "short_window_quota"
	default:
		return "rate_limit"
	}
}

func (worker Worker) recordJob(jobType, outcome string, duration time.Duration) {
	if worker.Metrics != nil {
		worker.Metrics.RecordWorkerJob(jobType, outcome, duration)
	}
}

func (worker Worker) recordDelivery(site, outcome string, duration time.Duration) {
	if worker.Metrics != nil {
		worker.Metrics.RecordDelivery(site, outcome, duration)
		worker.Metrics.RecordWorkerJob("site_delivery", outcome, duration)
	}
}

func (worker Worker) refreshMetrics(ctx context.Context) {
	queues, err := worker.Jobs.QueueStats(ctx)
	if err != nil {
		worker.log().Warn("failed to collect webhook queue metrics", "error", err)
	} else {
		worker.Metrics.SetQueueStats(queues.Webhooks.Depth, queues.Deliveries.Depth, queues.Syncs.Depth,
			queues.Webhooks.OldestAge, queues.Deliveries.OldestAge, queues.Syncs.OldestAge, queues.Stuck)
		worker.Metrics.SetDeadLetterStats(queues.Dead)
	}
	usage, err := worker.Artifacts.DiskUsage()
	if err != nil {
		worker.log().Warn("failed to collect artifact disk usage", "error", err)
	} else {
		worker.Metrics.SetArtifactDiskUsage(usage)
	}
	quotaSnapshots, err := worker.Quota.Snapshot(ctx)
	if err != nil {
		worker.log().Warn("failed to collect quota metrics", "error", err)
	} else {
		for _, snapshot := range quotaSnapshots {
			worker.Metrics.SetQuotaSnapshot(snapshot.Bucket, snapshot.Limit, snapshot.Used, snapshot.ResetAt)
		}
	}
}

func (worker Worker) logFailure(logger *slog.Logger, outcome string, err error) {
	var invalid invalidPayloadError
	if errors.As(err, &invalid) {
		logger = logger.With("stage", invalid.stage)
		if invalid.artifact != nil {
			logger = logger.With("artifact_sha256", invalid.artifact.SHA256, "artifact_path", invalid.artifact.RelativePath)
		}
	}
	if outcome == "failure" {
		logger.Error("Strava job retained in dead-letter queue", "disposition", "dead", "error", err)
		return
	}
	if isStatus(err, http.StatusTooManyRequests) {
		logger.Info("Strava job waiting for provider quota", "disposition", "retry", "error", err)
		return
	}
	logger.Warn("Strava job will retry", "disposition", "retry", "error", err)
}

func (worker Worker) processSync(ctx context.Context, job storage.SyncJob) error {
	connection, err := worker.Connections.Find(ctx, job.AthleteID)
	if err != nil {
		return err
	}
	if !provider.HasScope(connection.Scopes, "activity:read_all") {
		return errors.New("strava connection lacks activity:read_all")
	}
	connection, err = worker.refreshIfNeeded(ctx, connection, false)
	if err != nil {
		if isStatus(err, http.StatusUnauthorized) {
			return worker.failUnauthorizedSync(ctx, job, job.AthleteID, err)
		}
		return err
	}
	ids, err := worker.listActivities(ctx, connection, job)
	if classifyActivityFailure(err, false) == activityFailureRefresh {
		connection, err = worker.refreshIfNeeded(ctx, connection, true)
		if isStatus(err, http.StatusUnauthorized) {
			return worker.failUnauthorizedSync(ctx, job, job.AthleteID, err)
		}
		if err == nil {
			ids, err = worker.listActivities(ctx, connection, job)
		}
	}
	if isStatus(err, http.StatusUnauthorized) {
		return worker.failUnauthorizedSync(ctx, job, job.AthleteID, err)
	}
	if err != nil {
		return err
	}
	return worker.Syncs.CompletePage(ctx, job, ids)
}

func (worker Worker) listActivities(ctx context.Context, connection storage.Connection, job storage.SyncJob) ([]int64, error) {
	if retryAt, err := worker.acquireQuota(ctx); err != nil {
		return nil, err
	} else if !retryAt.IsZero() {
		return nil, provider.HTTPError{StatusCode: http.StatusTooManyRequests, RetryAt: retryAt}
	}
	ids, response, err := worker.Provider.ListActivities(ctx, connection.AccessToken, job.Page, job.AfterEpoch)
	if reconcileErr := worker.Quota.Reconcile(ctx, response.Headers); reconcileErr != nil {
		return nil, reconcileErr
	}
	if err != nil && response.StatusCode == http.StatusOK {
		return nil, worker.retainInvalidPayload("activity_list", response.Body, err)
	}
	return ids, err
}

func (worker Worker) failUnauthorizedSync(ctx context.Context, job storage.SyncJob, athleteID int64, cause error) error {
	if err := worker.Connections.Revoke(ctx, athleteID); err != nil {
		return err
	}
	if err := worker.Syncs.Fail(ctx, job, cause.Error()); err != nil {
		return err
	}
	return terminalSyncError{err: cause}
}

func (worker Worker) processEvent(ctx context.Context, job storage.EventJob) error {
	event := job.Event
	if event.ObjectType == "athlete" {
		if event.Updates["authorized"] != "false" {
			return worker.Jobs.IgnoreEvent(ctx, job)
		}
		if err := worker.Connections.Revoke(ctx, event.OwnerID); err != nil {
			return err
		}
		return worker.Jobs.CompleteEvent(ctx, job, "deauthorize", nil)
	}
	if event.AspectType == "delete" {
		return worker.Jobs.CompleteEvent(ctx, job, "delete", nil)
	}
	connection, err := worker.Connections.Find(ctx, event.OwnerID)
	if err != nil {
		return err
	}
	if !provider.HasScope(connection.Scopes, "activity:read_all") {
		return errors.New("strava connection lacks activity:read_all")
	}
	connection, err = worker.refreshIfNeeded(ctx, connection, false)
	if err != nil {
		if isStatus(err, http.StatusUnauthorized) {
			return worker.deauthorizeEvent(ctx, job, event.OwnerID)
		}
		return err
	}
	if retryAt, err := worker.acquireQuota(ctx); err != nil {
		return err
	} else if !retryAt.IsZero() {
		return provider.HTTPError{StatusCode: http.StatusTooManyRequests, RetryAt: retryAt}
	}
	detail, err := worker.Provider.FetchActivity(ctx, connection.AccessToken, event.ObjectID)
	if reconcileErr := worker.Quota.Reconcile(ctx, detail.Headers); reconcileErr != nil {
		return reconcileErr
	}
	if classifyActivityFailure(err, false) == activityFailureRefresh {
		connection, err = worker.refreshIfNeeded(ctx, connection, true)
		if err != nil {
			if isStatus(err, http.StatusUnauthorized) {
				return worker.deauthorizeEvent(ctx, job, event.OwnerID)
			}
			return err
		}
		if retryAt, quotaErr := worker.acquireQuota(ctx); quotaErr != nil {
			return quotaErr
		} else if !retryAt.IsZero() {
			return provider.HTTPError{StatusCode: http.StatusTooManyRequests, RetryAt: retryAt}
		}
		detail, err = worker.Provider.FetchActivity(ctx, connection.AccessToken, event.ObjectID)
		if reconcileErr := worker.Quota.Reconcile(ctx, detail.Headers); reconcileErr != nil {
			return reconcileErr
		}
	}
	if err != nil {
		disposition := classifyActivityFailure(err, true)
		if disposition == activityFailureDelete {
			return worker.Jobs.CompleteEvent(ctx, job, "delete", nil)
		}
		if disposition == activityFailureRevoke {
			return worker.deauthorizeEvent(ctx, job, event.OwnerID)
		}
		return err
	}
	activity, err := provider.ParseActivity(detail.Body)
	if err != nil {
		return worker.retainInvalidPayload("activity", detail.Body, err)
	}
	if activity.ID != event.ObjectID {
		return worker.retainInvalidPayload("activity", detail.Body,
			fmt.Errorf("strava returned activity %d for event %d", activity.ID, event.ObjectID))
	}
	if !activity.Cycling() {
		return worker.Jobs.CompleteEvent(ctx, job, "delete", nil)
	}
	if retryAt, err := worker.acquireQuota(ctx); err != nil {
		return err
	} else if !retryAt.IsZero() {
		return provider.HTTPError{StatusCode: http.StatusTooManyRequests, RetryAt: retryAt}
	}
	streams, err := worker.Provider.FetchStreams(ctx, connection.AccessToken, event.ObjectID)
	if reconcileErr := worker.Quota.Reconcile(ctx, streams.Headers); reconcileErr != nil {
		return reconcileErr
	}
	if classifyActivityFailure(err, false) == activityFailureRefresh {
		connection, err = worker.refreshIfNeeded(ctx, connection, true)
		if err != nil {
			if isStatus(err, http.StatusUnauthorized) {
				return worker.deauthorizeEvent(ctx, job, event.OwnerID)
			}
			return err
		}
		if retryAt, quotaErr := worker.acquireQuota(ctx); quotaErr != nil {
			return quotaErr
		} else if !retryAt.IsZero() {
			return provider.HTTPError{StatusCode: http.StatusTooManyRequests, RetryAt: retryAt}
		}
		streams, err = worker.Provider.FetchStreams(ctx, connection.AccessToken, event.ObjectID)
		if reconcileErr := worker.Quota.Reconcile(ctx, streams.Headers); reconcileErr != nil {
			return reconcileErr
		}
	}
	streams, err = normalizeMissingStreams(streams, err)
	if err != nil {
		disposition := classifyActivityFailure(err, true)
		if disposition == activityFailureDelete {
			return worker.Jobs.CompleteEvent(ctx, job, "delete", nil)
		}
		if disposition == activityFailureRevoke {
			return worker.deauthorizeEvent(ctx, job, event.OwnerID)
		}
		return err
	}
	if !json.Valid(streams.Body) {
		return worker.retainInvalidPayload("streams", streams.Body, errors.New("strava returned invalid streams JSON"))
	}
	payload, err := json.Marshal(struct {
		Activity json.RawMessage `json:"activity"`
		Streams  json.RawMessage `json:"streams"`
	}{detail.Body, streams.Body})
	if err != nil {
		return err
	}
	artifact, created, err := worker.Artifacts.WriteWithStatus(payload)
	if err != nil {
		if worker.Metrics != nil {
			worker.Metrics.RecordArtifactFailure("write")
		}
		return err
	}
	if created && worker.Metrics != nil {
		worker.Metrics.RecordArtifact(artifact.SizeBytes)
	}
	return worker.Jobs.CompleteEvent(ctx, job, "upsert", &artifact)
}

func (worker Worker) acquireQuota(ctx context.Context) (time.Time, error) {
	retryAt, err := worker.Quota.Acquire(ctx)
	if err != nil || retryAt.IsZero() || worker.Metrics == nil {
		return retryAt, err
	}
	buckets, bucketErr := worker.Quota.BlockedBuckets(ctx)
	if bucketErr != nil || len(buckets) == 0 {
		worker.Metrics.RecordQuotaPause("other")
		return retryAt, nil
	}
	for _, bucket := range buckets {
		worker.Metrics.RecordQuotaPause(bucket)
	}
	return retryAt, nil
}

func (worker Worker) refreshIfNeeded(ctx context.Context, connection storage.Connection, force bool) (storage.Connection, error) {
	if !force && connection.ExpiresAt.After(time.Now().Add(2*time.Minute)) {
		return connection, nil
	}
	if retryAt, err := worker.Quota.Acquire(ctx); err != nil {
		return storage.Connection{}, err
	} else if !retryAt.IsZero() {
		return storage.Connection{}, provider.HTTPError{StatusCode: http.StatusTooManyRequests, RetryAt: retryAt}
	}
	token, response, err := worker.Provider.Refresh(ctx, connection.RefreshToken)
	if reconcileErr := worker.Quota.Reconcile(ctx, response.Headers); reconcileErr != nil {
		return storage.Connection{}, reconcileErr
	}
	if err != nil {
		return storage.Connection{}, err
	}
	connection.AccessToken, connection.RefreshToken = token.AccessToken, token.RefreshToken
	connection.ExpiresAt = time.Unix(token.ExpiresAt, 0).UTC()
	if token.Scope != "" {
		connection.Scopes = provider.ParseScopes(token.Scope)
	}
	if err := worker.Connections.Upsert(ctx, connection); err != nil {
		return storage.Connection{}, err
	}
	return connection, nil
}

func (worker Worker) processDelivery(ctx context.Context, job storage.DeliveryJob) error {
	link, err := worker.Connections.FindLink(ctx, job.Event.OwnerID, job.Target)
	if errors.Is(err, storage.ErrLinkNotFound) {
		return worker.Jobs.CompleteDelivery(ctx, job)
	}
	if err != nil {
		return err
	}
	if err := worker.Sender.Send(ctx, job, link, worker.Artifacts); err != nil {
		return err
	}
	return worker.Jobs.CompleteDelivery(ctx, job)
}

func (worker Worker) deauthorizeEvent(ctx context.Context, job storage.EventJob, athleteID int64) error {
	if err := worker.Connections.Revoke(ctx, athleteID); err != nil {
		return err
	}
	return worker.Jobs.CompleteEvent(ctx, job, "deauthorize", nil)
}

func (worker Worker) log() *slog.Logger {
	if worker.Logger != nil {
		return worker.Logger
	}
	return slog.Default()
}

func isStatus(err error, status int) bool {
	var remote provider.HTTPError
	return errors.As(err, &remote) && remote.StatusCode == status
}

func retryTime(attempts int, err error) time.Time {
	var remote provider.HTTPError
	if errors.As(err, &remote) && remote.RetryAt.After(time.Now()) {
		return remote.RetryAt
	}
	if attempts > 12 {
		attempts = 12
	}
	delay := time.Duration(5*(1<<attempts)) * time.Second
	if delay > 6*time.Hour {
		delay = 6 * time.Hour
	}
	return time.Now().Add(delay + time.Duration(rand.Intn(5000))*time.Millisecond)
}
