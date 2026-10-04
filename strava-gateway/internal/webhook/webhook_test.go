package webhook

import (
	"context"
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/ericbutera/bike-services/strava-gateway/internal/observability"
)

type memoryInbox struct {
	keys map[string]bool
	err  error
}

func (inbox *memoryInbox) Store(_ context.Context, event Event, _ json.RawMessage) (bool, error) {
	if inbox.err != nil {
		return false, inbox.err
	}
	if inbox.keys == nil {
		inbox.keys = map[string]bool{}
	}
	_, exists := inbox.keys[event.Key()]
	inbox.keys[event.Key()] = true
	return !exists, nil
}

const eventJSON = `{"aspect_type":"create","event_time":1790500000,"object_id":42,"object_type":"activity","owner_id":8,"subscription_id":12,"updates":{}}`

func TestWebhookVerification(t *testing.T) {
	handler := Handler{VerifyToken: "secret", SubscriptionID: 12}
	good := httptest.NewRequest(http.MethodGet, "/webhooks/strava?hub.mode=subscribe&hub.verify_token=secret&hub.challenge=abc", nil)
	response := httptest.NewRecorder()
	handler.Webhook(response, good)
	if response.Code != http.StatusOK || strings.TrimSpace(response.Body.String()) != `{"hub.challenge":"abc"}` {
		t.Fatalf("verification: %d %s", response.Code, response.Body.String())
	}
	bad := httptest.NewRequest(http.MethodGet, "/webhooks/strava?hub.mode=subscribe&hub.verify_token=wrong&hub.challenge=abc", nil)
	response = httptest.NewRecorder()
	handler.Webhook(response, bad)
	if response.Code != http.StatusForbidden {
		t.Fatalf("wrong token returned %d", response.Code)
	}
}

func TestWebhookAcknowledgesStoredRetriesAndRejectsUnstoredEvents(t *testing.T) {
	inbox := &memoryInbox{}
	handler := Handler{Inbox: inbox, SubscriptionID: 12, AllowUnsigned: true}
	headers := []string{"", "t=1790500000,v1=" + strings.Repeat("0", 64), "unsupported-signature"}
	for _, header := range headers {
		response := post(t, handler, eventJSON, header)
		if response.Code != http.StatusOK {
			t.Fatalf("stored event returned %d: %s", response.Code, response.Body.String())
		}
	}
	if len(inbox.keys) != 1 {
		t.Fatalf("duplicate produced %d keys", len(inbox.keys))
	}
	inbox.err = errors.New("database down")
	for _, header := range headers {
		if response := post(t, handler, eventJSON, header); response.Code != http.StatusServiceUnavailable {
			t.Fatalf("unpersisted event returned %d", response.Code)
		}
		if response := post(t, handler, strings.Replace(eventJSON, `"subscription_id":12`, `"subscription_id":13`, 1), header); response.Code != http.StatusBadRequest {
			t.Fatalf("wrong subscription returned %d", response.Code)
		}
	}
	handler.AllowUnsigned = false
	for _, header := range headers {
		if response := post(t, handler, eventJSON, header); response.Code != http.StatusForbidden {
			t.Fatalf("implicit unsigned mode returned %d", response.Code)
		}
	}
}

func TestWebhookSignatureAndCanonicalKey(t *testing.T) {
	now := time.Unix(1790500000, 0)
	inbox := &memoryInbox{}
	handler := Handler{Inbox: inbox, SubscriptionID: 12, SigningSecret: "signing-secret", Now: func() time.Time { return now }}
	timestamp := strconv.FormatInt(now.Unix(), 10)
	mac := hmac.New(sha256.New, []byte("signing-secret"))
	_, _ = mac.Write([]byte(timestamp + "." + eventJSON))
	signature := "t=" + timestamp + ",v1=" + hex.EncodeToString(mac.Sum(nil))
	if response := post(t, handler, eventJSON, signature); response.Code != http.StatusOK {
		t.Fatalf("valid signature returned %d: %s", response.Code, response.Body.String())
	}
	for _, invalid := range []string{"", "t=1,v1=" + hex.EncodeToString(mac.Sum(nil)), "t=" + timestamp + ",v1=bad"} {
		if response := post(t, handler, eventJSON, invalid); response.Code != http.StatusForbidden {
			t.Fatalf("invalid signature %q returned %d", invalid, response.Code)
		}
	}
	handler.AllowUnsigned = true
	if response := post(t, handler, eventJSON, ""); response.Code != http.StatusOK {
		t.Fatalf("explicit unsigned mode with configured key returned %d", response.Code)
	}
	if response := post(t, handler, eventJSON, "t="+timestamp+",v1=bad"); response.Code != http.StatusForbidden {
		t.Fatalf("invalid signature with configured key returned %d", response.Code)
	}
	var first, second Event
	_ = json.Unmarshal([]byte(eventJSON), &first)
	_ = json.Unmarshal([]byte(`{"updates":{},"subscription_id":12,"owner_id":8,"object_type":"activity","object_id":42,"event_time":1790500000,"aspect_type":"create"}`), &second)
	if first.Key() != second.Key() {
		t.Fatal("equivalent events produced different dedupe keys")
	}
}

func TestWebhookLogsRejectionReasonsWithoutRequestSecrets(t *testing.T) {
	var output strings.Builder
	previous := slog.Default()
	slog.SetDefault(observability.NewLogger("bike-strava-gateway", &output))
	t.Cleanup(func() { slog.SetDefault(previous) })
	cases := []struct {
		reason, body, signingSecret string
		inboxError                  error
		status                      int
	}{
		{reason: "invalid_json", body: `{"private":"private-body"`, status: http.StatusBadRequest},
		{reason: "subscription_mismatch", body: strings.Replace(eventJSON, `"subscription_id":12`, `"subscription_id":13`, 1), status: http.StatusBadRequest},
		{reason: "invalid_signature", body: eventJSON, signingSecret: "private-signing-key", status: http.StatusForbidden},
		{reason: "inbox_unavailable", body: eventJSON, inboxError: errors.New("database down"), status: http.StatusServiceUnavailable},
	}
	for _, item := range cases {
		t.Run(item.reason, func(t *testing.T) {
			output.Reset()
			handler := Handler{Inbox: &memoryInbox{err: item.inboxError}, SubscriptionID: 12, AllowUnsigned: true, SigningSecret: item.signingSecret}
			request := httptest.NewRequest(http.MethodPost, "/webhooks/strava?hub.verify_token=private-token&hub.challenge=private-challenge", strings.NewReader(item.body))
			request.Header.Set("Content-Type", "application/json")
			request.Header.Set("X-Strava-Signature", "private-signature")
			request.Header.Set("Authorization", "Bearer private-authorization")
			response := httptest.NewRecorder()
			handler.Webhook(response, request)
			if response.Code != item.status {
				t.Fatalf("status = %d, want %d", response.Code, item.status)
			}
			var fields map[string]any
			if err := json.Unmarshal([]byte(output.String()), &fields); err != nil {
				t.Fatal(err)
			}
			if fields["msg"] != "Strava webhook rejected" || fields["reason"] != item.reason || fields["status_code"] != float64(item.status) {
				t.Fatalf("missing rejection detail: %v", fields)
			}
			if fields["signature_present"] != true || fields["has_verify_token"] != true || fields["has_challenge"] != true {
				t.Fatalf("missing presence flags: %v", fields)
			}
			if strings.Contains(output.String(), "private-") {
				t.Fatal("rejection log contains a request secret or body")
			}
		})
	}
	output.Reset()
	handler := Handler{Inbox: &memoryInbox{}, SubscriptionID: 12, AllowUnsigned: true}
	for _, outcome := range []string{"accepted", "duplicate"} {
		output.Reset()
		if response := post(t, handler, eventJSON, ""); response.Code != http.StatusOK {
			t.Fatal(response.Code)
		}
		var fields map[string]any
		if err := json.Unmarshal([]byte(output.String()), &fields); err != nil {
			t.Fatal(err)
		}
		if fields["outcome"] != outcome || fields["object_id"] != float64(42) || fields["event_key"] == "" {
			t.Fatalf("missing stored-event detail: %v", fields)
		}
	}
}

func post(t *testing.T, handler Handler, body, signature string) *httptest.ResponseRecorder {
	t.Helper()
	request := httptest.NewRequest(http.MethodPost, "/webhooks/strava", strings.NewReader(body))
	request.Header.Set("Content-Type", "application/json")
	if signature != "" {
		request.Header.Set("X-Strava-Signature", signature)
	}
	response := httptest.NewRecorder()
	handler.Webhook(response, request)
	return response
}
