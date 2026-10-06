package oauth

import (
	"context"
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"time"

	"github.com/ericbutera/bike-services/strava-gateway/internal/provider"
	"github.com/ericbutera/bike-services/strava-gateway/internal/storage"
)

type Handler struct {
	States      storage.OAuthStates
	Syncs       storage.Syncs
	Connections storage.Connections
	Quota       storage.Quota
	Provider    provider.OAuthAPI
	CallbackURL string
	Sites       map[string]Site
	Now         func() time.Time
}

type Site struct {
	Secret    string
	ReturnURL string
}

type IntentRequest struct {
	Target     string `json:"target"`
	SiteUserID int64  `json:"site_user_id"`
	Mode       string `json:"mode,omitempty"`
}

func (handler Handler) Routes(mux *http.ServeMux) {
	mux.HandleFunc("POST /v1/oauth/intents", handler.CreateIntent)
	mux.HandleFunc("POST /v1/connections/status", handler.ConnectionStatus)
	mux.HandleFunc("POST /v1/connections/disconnect", handler.Disconnect)
	mux.HandleFunc("POST /v1/sync", handler.Sync)
	mux.HandleFunc("GET /oauth/callback", handler.Callback)
}

func (handler Handler) ConnectionStatus(w http.ResponseWriter, r *http.Request) {
	input, ok := handler.siteRequest(w, r)
	if !ok {
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 3*time.Second)
	defer cancel()
	link, err := handler.Connections.FindLinkByUser(ctx, input.Target, input.SiteUserID)
	if errors.Is(err, storage.ErrLinkNotFound) {
		writeJSON(w, map[string]any{
			"configured": true, "connected": false, "scopes": []string{},
			"last_sync_status": "never", "last_sync_imported_count": 0,
			"last_sync_duplicate_count": 0, "last_sync_failed_count": 0,
			"last_sync_wait_reason": nil, "last_sync_next_attempt_at": nil,
		})
		return
	}
	if err != nil {
		http.Error(w, "connection unavailable", http.StatusServiceUnavailable)
		return
	}
	connection, err := handler.Connections.Find(ctx, link.AthleteID)
	if err != nil {
		http.Error(w, "connection unavailable", http.StatusServiceUnavailable)
		return
	}
	status, err := handler.Syncs.Status(ctx, link)
	if err != nil {
		http.Error(w, "sync status unavailable", http.StatusServiceUnavailable)
		return
	}
	writeJSON(w, map[string]any{
		"configured": true, "connected": true, "athlete_id": link.AthleteID,
		"scopes": connection.Scopes, "last_sync_status": status.Status,
		"last_sync_wait_reason":     status.WaitingReason,
		"last_sync_next_attempt_at": status.NextAttemptAt,
		"last_sync_imported_count":  0, "last_sync_duplicate_count": 0,
		"last_sync_failed_count": 0,
	})
}

func (handler Handler) Sync(w http.ResponseWriter, r *http.Request) {
	input, ok := handler.siteRequest(w, r)
	if !ok {
		return
	}
	if input.Mode != "initial" && input.Mode != "incremental" && input.Mode != "full" {
		http.Error(w, "invalid sync mode", http.StatusBadRequest)
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 3*time.Second)
	defer cancel()
	link, err := handler.Connections.FindLinkByUser(ctx, input.Target, input.SiteUserID)
	if errors.Is(err, storage.ErrLinkNotFound) {
		http.Error(w, "Strava is not connected", http.StatusNotFound)
		return
	}
	if err != nil || handler.Syncs.Queue(ctx, link, input.Mode) != nil {
		http.Error(w, "sync unavailable", http.StatusServiceUnavailable)
		return
	}
	writeJSON(w, map[string]string{"status": "queued"})
}

func (handler Handler) Disconnect(w http.ResponseWriter, r *http.Request) {
	input, ok := handler.siteRequest(w, r)
	if !ok {
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 3*time.Second)
	defer cancel()
	link, err := handler.Connections.FindLinkByUser(ctx, input.Target, input.SiteUserID)
	if errors.Is(err, storage.ErrLinkNotFound) {
		writeJSON(w, map[string]string{"status": "disconnected"})
		return
	}
	if err != nil || handler.Connections.DisableLink(ctx, link) != nil {
		http.Error(w, "disconnect unavailable", http.StatusServiceUnavailable)
		return
	}
	writeJSON(w, map[string]string{"status": "disconnected"})
}

func (handler Handler) siteRequest(w http.ResponseWriter, r *http.Request) (IntentRequest, bool) {
	body, err := io.ReadAll(http.MaxBytesReader(w, r.Body, 4096))
	if err != nil {
		http.Error(w, "request too large", http.StatusRequestEntityTooLarge)
		return IntentRequest{}, false
	}
	var input IntentRequest
	if err := json.Unmarshal(body, &input); err != nil || input.SiteUserID <= 0 {
		http.Error(w, "invalid site request", http.StatusBadRequest)
		return IntentRequest{}, false
	}
	site, exists := handler.Sites[input.Target]
	if !exists || site.Secret == "" || !handler.validSiteSignature(r, body, site.Secret) {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return IntentRequest{}, false
	}
	return input, true
}

func writeJSON(w http.ResponseWriter, value any) {
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(value)
}

func (handler Handler) CreateIntent(w http.ResponseWriter, r *http.Request) {
	body, err := io.ReadAll(http.MaxBytesReader(w, r.Body, 4096))
	if err != nil {
		http.Error(w, "request too large", http.StatusRequestEntityTooLarge)
		return
	}
	var input IntentRequest
	if err := json.Unmarshal(body, &input); err != nil || input.SiteUserID <= 0 {
		http.Error(w, "invalid intent", http.StatusBadRequest)
		return
	}
	site, exists := handler.Sites[input.Target]
	if !exists || site.Secret == "" || site.ReturnURL == "" ||
		!handler.validSiteSignature(r, body, site.Secret) {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	stateBytes := make([]byte, 32)
	if _, err := rand.Read(stateBytes); err != nil {
		http.Error(w, "state unavailable", http.StatusServiceUnavailable)
		return
	}
	state := base64.RawURLEncoding.EncodeToString(stateBytes)
	ctx, cancel := context.WithTimeout(r.Context(), 3*time.Second)
	defer cancel()
	if err := handler.States.Save(ctx, state, storage.OAuthState{
		Target: input.Target, UserID: input.SiteUserID,
	}); err != nil {
		http.Error(w, "state unavailable", http.StatusServiceUnavailable)
		return
	}
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(map[string]string{
		"authorization_url": handler.Provider.AuthorizationURL(handler.CallbackURL, state),
	})
}

func (handler Handler) Callback(w http.ResponseWriter, r *http.Request) {
	state := r.URL.Query().Get("state")
	code := r.URL.Query().Get("code")
	if state == "" {
		http.Error(w, "missing OAuth state", http.StatusBadRequest)
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 15*time.Second)
	defer cancel()
	site, err := handler.States.Consume(ctx, state)
	if err != nil {
		http.Error(w, "invalid OAuth state", http.StatusBadRequest)
		return
	}
	returnSite, exists := handler.Sites[site.Target]
	if !exists {
		http.Error(w, "unknown OAuth site", http.StatusBadRequest)
		return
	}
	status := "error"
	if code != "" && r.URL.Query().Get("error") == "" {
		if err := handler.complete(ctx, code, site); err == nil {
			status = "connected"
		}
	}
	destination, err := url.Parse(returnSite.ReturnURL)
	if err != nil || destination.Scheme != "https" {
		http.Error(w, "invalid OAuth return URL", http.StatusInternalServerError)
		return
	}
	query := destination.Query()
	query.Set("strava", status)
	destination.RawQuery = query.Encode()
	http.Redirect(w, r, destination.String(), http.StatusSeeOther)
}

func (handler Handler) complete(ctx context.Context, code string, site storage.OAuthState) error {
	if retryAt, err := handler.Quota.Acquire(ctx); err != nil {
		return err
	} else if !retryAt.IsZero() {
		return provider.HTTPError{StatusCode: http.StatusTooManyRequests, RetryAt: retryAt}
	}
	token, response, err := handler.Provider.ExchangeCode(ctx, code)
	if reconcileErr := handler.Quota.Reconcile(ctx, response.Headers); reconcileErr != nil {
		return reconcileErr
	}
	if err != nil {
		return err
	}
	if !provider.HasScope(provider.ParseScopes(token.Scope), "activity:read_all") {
		return errors.New("strava did not grant activity:read_all")
	}
	athleteID := token.Athlete.ID
	if athleteID <= 0 {
		if retryAt, err := handler.Quota.Acquire(ctx); err != nil {
			return err
		} else if !retryAt.IsZero() {
			return provider.HTTPError{StatusCode: http.StatusTooManyRequests, RetryAt: retryAt}
		}
		var athleteResponse provider.Response
		athleteID, athleteResponse, err = handler.Provider.AuthenticatedAthlete(ctx, token.AccessToken)
		if reconcileErr := handler.Quota.Reconcile(ctx, athleteResponse.Headers); reconcileErr != nil {
			return reconcileErr
		}
		if err != nil {
			return err
		}
	}
	if err := handler.Connections.Upsert(ctx, storage.Connection{
		AthleteID: athleteID, AccessToken: token.AccessToken,
		RefreshToken: token.RefreshToken, ExpiresAt: time.Unix(token.ExpiresAt, 0).UTC(),
		Scopes: provider.ParseScopes(token.Scope),
	}); err != nil {
		return err
	}
	link := storage.SiteLink{
		AthleteID: athleteID, Target: site.Target, UserID: site.UserID,
	}
	if err := handler.Connections.Link(ctx, link); err != nil {
		return err
	}
	return handler.Syncs.Queue(ctx, link, "initial")
}

func (handler Handler) validSiteSignature(r *http.Request, body []byte, secret string) bool {
	timestamp := r.Header.Get("X-Bike-Request-Timestamp")
	provided, err := hex.DecodeString(r.Header.Get("X-Bike-Request-Signature"))
	if err != nil || len(provided) != sha256.Size {
		return false
	}
	seconds, err := strconv.ParseInt(timestamp, 10, 64)
	if err != nil {
		return false
	}
	now := time.Now()
	if handler.Now != nil {
		now = handler.Now()
	}
	if delta := now.Unix() - seconds; delta > 300 || delta < -300 {
		return false
	}
	mac := hmac.New(sha256.New, []byte(secret))
	_, _ = io.WriteString(mac, timestamp+"\n"+r.Method+" "+r.URL.Path+"\n")
	_, _ = mac.Write(body)
	return hmac.Equal(provided, mac.Sum(nil))
}
