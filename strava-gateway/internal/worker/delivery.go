package worker

import (
	"bytes"
	"context"
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"time"

	"github.com/ericbutera/bike/strava-gateway/internal/storage"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/propagation"
)

type Target struct {
	URL    string
	Secret string
}

type DeliverySender struct {
	HTTP    *http.Client
	Targets map[string]Target
}

type Envelope struct {
	Version       int             `json:"version"`
	DeliveryID    string          `json:"delivery_id"`
	AthleteID     int64           `json:"athlete_id"`
	SiteUserID    int64           `json:"site_user_id"`
	ActivityID    int64           `json:"activity_id"`
	EventTime     int64           `json:"event_time"`
	Operation     string          `json:"operation"`
	ContentSHA256 string          `json:"content_sha256,omitempty"`
	Payload       json.RawMessage `json:"payload,omitempty"`
	Pipeline      *PipelineOrigin `json:"pipeline,omitempty"`
}

type PipelineOrigin struct {
	RunID          string            `json:"run_id"`
	StartedAt      time.Time         `json:"pipeline_started_at"`
	Entrypoint     string            `json:"entrypoint"`
	RequestID      *string           `json:"request_id"`
	ParentTaskID   *int              `json:"parent_task_id"`
	TraceContext   map[string]string `json:"trace_context"`
	GatewayHistory []GatewayStep     `json:"gateway_history"`
}
type GatewayStep struct {
	Kind       string    `json:"kind"`
	StartedAt  time.Time `json:"started_at"`
	FinishedAt time.Time `json:"finished_at"`
	Attempts   int       `json:"attempts"`
}

func pipelineOrigin(ctx context.Context, id string, job storage.DeliveryJob) *PipelineOrigin {
	if job.ReceivedAt.IsZero() {
		return nil
	} // Legacy evidence stays unknown.
	digest := sha256.Sum256([]byte("bike:strava-delivery:" + id))
	digest[6] = (digest[6] & 0x0f) | 0x50
	digest[8] = (digest[8] & 0x3f) | 0x80
	carrier := propagation.MapCarrier{}
	otel.GetTextMapPropagator().Inject(ctx, carrier)
	entrypoint := "strava_webhook"
	if job.Event.SubscriptionID == 0 {
		entrypoint = "strava_sync"
	}
	return &PipelineOrigin{
		RunID:     fmt.Sprintf("%x-%x-%x-%x-%x", digest[0:4], digest[4:6], digest[6:8], digest[8:10], digest[10:16]),
		StartedAt: job.ReceivedAt, Entrypoint: entrypoint, TraceContext: carrier,
		GatewayHistory: []GatewayStep{
			{Kind: "gateway_receive_to_fetch", StartedAt: job.ReceivedAt, FinishedAt: job.FetchedAt},
			{Kind: "gateway_delivery_wait", StartedAt: job.CreatedAt, FinishedAt: job.ClaimedAt, Attempts: job.Attempts},
		},
	}
}

type DeliveryError struct {
	StatusCode int
	Reason     string
}

func (err DeliveryError) Error() string { return err.Reason }

func (sender DeliverySender) Send(ctx context.Context, job storage.DeliveryJob, link storage.SiteLink, artifacts Artifacts) (err error) {
	target, exists := sender.Targets[job.Target]
	if !exists || target.URL == "" || target.Secret == "" {
		return errors.New("delivery target is not configured")
	}
	id := job.Target + ":" + strconv.FormatInt(job.ID, 10)
	envelope := Envelope{
		Version: 1, DeliveryID: id, AthleteID: job.Event.OwnerID,
		SiteUserID: link.UserID, ActivityID: job.Event.ObjectID,
		EventTime: job.Event.EventTime, Operation: job.Operation,
		Pipeline: pipelineOrigin(ctx, id, job),
	}
	if job.Operation == "upsert" {
		if job.Artifact == nil {
			return errors.New("delivery has no fetched artifact")
		}
		payload, err := artifacts.Read(*job.Artifact)
		if err != nil {
			return err
		}
		envelope.ContentSHA256 = job.Artifact.SHA256
		envelope.Payload = payload
	}
	body, err := json.Marshal(envelope)
	if err != nil {
		return err
	}
	timestamp := strconv.FormatInt(time.Now().Unix(), 10)
	mac := hmac.New(sha256.New, []byte(target.Secret))
	_, _ = io.WriteString(mac, timestamp+"\n"+id+"\n")
	_, _ = mac.Write(body)
	request, err := http.NewRequestWithContext(ctx, http.MethodPost, target.URL, bytes.NewReader(body))
	if err != nil {
		return err
	}
	request.Header.Set("Content-Type", "application/json")
	request.Header.Set("X-Bike-Delivery-ID", id)
	request.Header.Set("X-Bike-Delivery-Timestamp", timestamp)
	request.Header.Set("X-Bike-Delivery-Signature", hex.EncodeToString(mac.Sum(nil)))
	httpClient := sender.HTTP
	if httpClient == nil {
		httpClient = &http.Client{Timeout: 15 * time.Second}
	}
	response, err := httpClient.Do(request)
	if err != nil {
		return err
	}
	defer func() { err = errors.Join(err, response.Body.Close()) }()
	if _, err := io.Copy(io.Discard, io.LimitReader(response.Body, 4096)); err != nil {
		return err
	}
	if response.StatusCode == http.StatusOK || response.StatusCode == http.StatusAccepted {
		return nil
	}
	return DeliveryError{StatusCode: response.StatusCode,
		Reason: fmt.Sprintf("%s receiver returned HTTP %d", job.Target, response.StatusCode)}
}
