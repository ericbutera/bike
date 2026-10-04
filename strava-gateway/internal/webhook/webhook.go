package webhook

import (
	"context"
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
	"log/slog"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/ericbutera/bike-services/strava-gateway/internal/observability"
	"go.opentelemetry.io/otel/trace"
)

const maxBodyBytes = 32 << 10

type Event struct {
	AspectType     string            `json:"aspect_type"`
	EventTime      int64             `json:"event_time"`
	ObjectID       int64             `json:"object_id"`
	ObjectType     string            `json:"object_type"`
	OwnerID        int64             `json:"owner_id"`
	SubscriptionID int64             `json:"subscription_id"`
	Updates        map[string]string `json:"updates"`
}

func (event Event) Valid(subscriptionID int64) bool {
	if event.SubscriptionID != subscriptionID || event.OwnerID <= 0 || event.ObjectID <= 0 || event.EventTime <= 0 {
		return false
	}
	if event.ObjectType != "activity" && event.ObjectType != "athlete" {
		return false
	}
	return event.AspectType == "create" || event.AspectType == "update" || event.AspectType == "delete"
}

// Key canonicalizes known Strava fields. JSON map keys are sorted by the Go
// encoder, so an identical retry deduplicates even if object key order changes.
func (event Event) Key() string {
	payload, _ := json.Marshal(event)
	sum := sha256.Sum256(payload)
	return hex.EncodeToString(sum[:])
}

type Inbox interface {
	Store(context.Context, Event, json.RawMessage) (bool, error)
}

type Handler struct {
	Inbox          Inbox
	VerifyToken    string
	SubscriptionID int64
	SigningSecret  string
	AllowUnsigned  bool
	Now            func() time.Time
	Metrics        *observability.Metrics
}

func (handler Handler) Routes() http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("/webhooks/strava", handler.Webhook)
	return mux
}

func (handler Handler) Webhook(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet:
		handler.verify(w, r)
	case http.MethodPost:
		handler.receive(w, r)
	default:
		w.Header().Set("Allow", "GET, POST")
		handler.reject(w, r, http.StatusMethodNotAllowed, "unsupported_method", "method not allowed")
	}
}

func (handler Handler) verify(w http.ResponseWriter, r *http.Request) {
	query := r.URL.Query()
	challenge := query.Get("hub.challenge")
	if query.Get("hub.mode") != "subscribe" || handler.VerifyToken == "" ||
		!hmac.Equal([]byte(query.Get("hub.verify_token")), []byte(handler.VerifyToken)) ||
		challenge == "" || len(challenge) > 512 {
		handler.reject(w, r, http.StatusForbidden, "invalid_verification", "invalid verification request")
		return
	}
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(map[string]string{"hub.challenge": challenge})
}

func (handler Handler) receive(w http.ResponseWriter, r *http.Request) {
	outcome := "rejected"
	defer func() {
		if handler.Metrics != nil {
			handler.Metrics.RecordWebhookOutcome(outcome)
		}
	}()
	if handler.Inbox == nil || handler.SubscriptionID <= 0 {
		outcome = "unavailable"
		handler.reject(w, r, http.StatusServiceUnavailable, "handler_unavailable", "webhook is unavailable")
		return
	}
	if !strings.HasPrefix(r.Header.Get("Content-Type"), "application/json") {
		handler.reject(w, r, http.StatusUnsupportedMediaType, "unsupported_content_type", "JSON required")
		return
	}
	data, err := io.ReadAll(http.MaxBytesReader(w, r.Body, maxBodyBytes))
	if err != nil {
		outcome = "invalid"
		handler.reject(w, r, http.StatusRequestEntityTooLarge, "invalid_body", "invalid body")
		return
	}
	if !handler.signatureValid(r.Header.Get("X-Strava-Signature"), data) {
		outcome = "invalid"
		handler.reject(w, r, http.StatusForbidden, "invalid_signature", "invalid signature")
		return
	}
	var event Event
	if err := json.Unmarshal(data, &event); err != nil {
		outcome = "invalid"
		handler.reject(w, r, http.StatusBadRequest, "invalid_json", "invalid event")
		return
	}
	if !event.Valid(handler.SubscriptionID) {
		outcome = "invalid"
		reason := "invalid_event"
		if event.SubscriptionID != handler.SubscriptionID {
			reason = "subscription_mismatch"
		}
		handler.reject(w, r, http.StatusBadRequest, reason, "invalid event")
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 1500*time.Millisecond)
	defer cancel()
	inserted, err := handler.Inbox.Store(ctx, event, data)
	if err != nil {
		outcome = "unavailable"
		handler.reject(w, r, http.StatusServiceUnavailable, "inbox_unavailable", "inbox unavailable")
		return
	}
	if inserted {
		outcome = "accepted"
	} else {
		outcome = "duplicate"
	}
	observability.WithSpanFields(slog.Default(), trace.SpanFromContext(r.Context())).InfoContext(r.Context(),
		"Strava webhook stored", "outcome", outcome, "event_key", event.Key(),
		"owner_id", event.OwnerID, "object_id", event.ObjectID,
		"object_type", event.ObjectType, "aspect_type", event.AspectType)
	w.Header().Set("Content-Type", "application/json")
	_, _ = io.WriteString(w, `{"message":"ok"}`)
}

func (handler Handler) reject(w http.ResponseWriter, r *http.Request, status int, reason, message string) {
	logger := observability.WithSpanFields(slog.Default(), trace.SpanFromContext(r.Context()))
	level := slog.LevelWarn
	if status >= http.StatusInternalServerError {
		level = slog.LevelError
	}
	// Log bounded reasons and presence flags, never request bodies, query values,
	// authorization headers, verification tokens, or signature contents.
	logger.Log(r.Context(), level, "Strava webhook rejected",
		"reason", reason, "status_code", status, "method", r.Method,
		"body_bytes", r.ContentLength, "signature_present", r.Header.Get("X-Strava-Signature") != "",
		"has_challenge", r.URL.Query().Has("hub.challenge"),
		"has_verify_token", r.URL.Query().Has("hub.verify_token"))
	http.Error(w, message, status)
}

func (handler Handler) signatureValid(header string, data []byte) bool {
	// Without a supported signing key, explicit unsigned mode also accepts
	// callbacks carrying Strava's currently unverifiable signature header.
	if handler.SigningSecret == "" || header == "" {
		return handler.AllowUnsigned
	}
	parts := map[string]string{}
	for _, part := range strings.Split(header, ",") {
		key, value, ok := strings.Cut(strings.TrimSpace(part), "=")
		if !ok || parts[key] != "" {
			return false
		}
		parts[key] = value
	}
	timestamp, err := strconv.ParseInt(parts["t"], 10, 64)
	if err != nil || len(parts["v1"]) != 64 {
		return false
	}
	now := time.Now()
	if handler.Now != nil {
		now = handler.Now()
	}
	if delta := now.Unix() - timestamp; delta > 300 || delta < -300 {
		return false
	}
	provided, err := hex.DecodeString(parts["v1"])
	if err != nil {
		return false
	}
	mac := hmac.New(sha256.New, []byte(handler.SigningSecret))
	_, _ = io.WriteString(mac, parts["t"]+".")
	_, _ = mac.Write(data)
	return hmac.Equal(provided, mac.Sum(nil))
}

var ErrInvalidConfiguration = errors.New("webhook requires a signing secret or explicit unsigned mode")

func (handler Handler) Validate() error {
	if handler.VerifyToken == "" || handler.SubscriptionID <= 0 || (handler.SigningSecret == "" && !handler.AllowUnsigned) {
		return ErrInvalidConfiguration
	}
	return nil
}
