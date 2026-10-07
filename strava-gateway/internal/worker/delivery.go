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
