package provider

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"

	"github.com/ericbutera/bike-services/strava-gateway/internal/observability"
)

const maxProviderBody = 32 << 20

type Client struct {
	HTTP         *http.Client
	APIBaseURL   string
	AuthBaseURL  string
	ClientID     string
	ClientSecret string
	Metrics      *observability.Metrics
}

func (client Client) AuthorizationURL(redirectURL, state string) string {
	base := strings.TrimRight(client.AuthBaseURL, "/")
	if base == "" {
		base = "https://www.strava.com"
	}
	query := url.Values{
		"client_id": {client.ClientID}, "redirect_uri": {redirectURL},
		"response_type": {"code"}, "approval_prompt": {"auto"},
		"scope": {"activity:read_all"}, "state": {state},
	}
	return base + "/oauth/authorize?" + query.Encode()
}

func (client Client) AuthenticatedAthlete(ctx context.Context, token string) (int64, Response, error) {
	response, err := client.get(ctx, token, "/athlete")
	if err != nil {
		return 0, response, err
	}
	var athlete struct {
		ID int64 `json:"id"`
	}
	if err := json.Unmarshal(response.Body, &athlete); err != nil {
		return 0, response, err
	}
	if athlete.ID <= 0 {
		return 0, response, errors.New("Strava athlete response omitted ID")
	}
	return athlete.ID, response, nil
}

type Token struct {
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	ExpiresAt    int64  `json:"expires_at"`
	Scope        string `json:"scope"`
	Athlete      struct {
		ID int64 `json:"id"`
	} `json:"athlete"`
}

type Response struct {
	Body       json.RawMessage
	StatusCode int
	Headers    http.Header
}

type HTTPError struct {
	StatusCode int
	RetryAt    time.Time
}

func (err HTTPError) Error() string { return fmt.Sprintf("Strava returned HTTP %d", err.StatusCode) }

type FailureKind uint8

const (
	FailureOther FailureKind = iota
	FailureUnauthorized
	FailureRateLimited
	FailureForbidden
	FailureNotFound
)

func (err HTTPError) Kind() FailureKind {
	switch err.StatusCode {
	case http.StatusUnauthorized:
		return FailureUnauthorized
	case http.StatusTooManyRequests:
		return FailureRateLimited
	case http.StatusForbidden:
		return FailureForbidden
	case http.StatusNotFound:
		return FailureNotFound
	default:
		return FailureOther
	}
}

func (client Client) FetchActivity(ctx context.Context, token string, activityID int64) (Response, error) {
	return client.get(ctx, token, "/activities/"+strconv.FormatInt(activityID, 10))
}

func (client Client) FetchStreams(ctx context.Context, token string, activityID int64) (Response, error) {
	path := "/activities/" + strconv.FormatInt(activityID, 10) + "/streams"
	query := url.Values{
		"keys":        {"time,distance,latlng,altitude,velocity_smooth,heartrate,cadence,watts,temp,moving,grade_smooth"},
		"key_by_type": {"true"},
	}
	return client.get(ctx, token, path+"?"+query.Encode())
}

func (client Client) ListActivities(ctx context.Context, token string, page int, afterEpoch int64) ([]int64, Response, error) {
	query := url.Values{"page": {strconv.Itoa(page)}, "per_page": {"100"}}
	if afterEpoch > 0 {
		query.Set("after", strconv.FormatInt(afterEpoch, 10))
	}
	response, err := client.get(ctx, token, "/athlete/activities?"+query.Encode())
	if err != nil {
		return nil, response, err
	}
	var values []struct {
		ID int64 `json:"id"`
	}
	if err := json.Unmarshal(response.Body, &values); err != nil {
		return nil, response, err
	}
	ids := make([]int64, 0, len(values))
	for _, value := range values {
		if value.ID > 0 {
			ids = append(ids, value.ID)
		}
	}
	return ids, response, nil
}

func (client Client) get(ctx context.Context, token, path string) (Response, error) {
	base := strings.TrimRight(client.APIBaseURL, "/")
	if base == "" {
		base = "https://www.strava.com/api/v3"
	}
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, base+path, nil)
	if err != nil {
		return Response{}, err
	}
	request.Header.Set("Authorization", "Bearer "+token)
	return client.send(request)
}

func (client Client) Refresh(ctx context.Context, refreshToken string) (Token, Response, error) {
	return client.exchange(ctx, url.Values{"refresh_token": {refreshToken}, "grant_type": {"refresh_token"}})
}

func (client Client) ExchangeCode(ctx context.Context, code string) (Token, Response, error) {
	return client.exchange(ctx, url.Values{"code": {code}, "grant_type": {"authorization_code"}})
}

func (client Client) exchange(ctx context.Context, values url.Values) (Token, Response, error) {
	if client.ClientID == "" || client.ClientSecret == "" {
		return Token{}, Response{}, errors.New("Strava client credentials are required")
	}
	values.Set("client_id", client.ClientID)
	values.Set("client_secret", client.ClientSecret)
	base := strings.TrimRight(client.AuthBaseURL, "/")
	if base == "" {
		base = "https://www.strava.com"
	}
	request, err := http.NewRequestWithContext(ctx, http.MethodPost,
		base+"/api/v3/oauth/token", strings.NewReader(values.Encode()))
	if err != nil {
		return Token{}, Response{}, err
	}
	request.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	response, err := client.send(request)
	if err != nil {
		return Token{}, response, err
	}
	var token Token
	if err := json.Unmarshal(response.Body, &token); err != nil {
		return Token{}, response, err
	}
	if token.AccessToken == "" || token.RefreshToken == "" || token.ExpiresAt <= 0 {
		return Token{}, response, errors.New("Strava returned incomplete tokens")
	}
	return token, response, nil
}

func (client Client) send(request *http.Request) (Response, error) {
	started := time.Now()
	httpClient := client.HTTP
	if httpClient == nil {
		httpClient = &http.Client{Timeout: 15 * time.Second}
	}
	remote, err := httpClient.Do(request)
	if err != nil {
		client.recordMetrics(request.URL.Path, 0, started)
		return Response{}, err
	}
	defer remote.Body.Close()
	if client.Metrics != nil {
		client.Metrics.RecordQuota(remote.Header)
	}
	body, err := io.ReadAll(io.LimitReader(remote.Body, maxProviderBody+1))
	if err != nil {
		client.recordMetrics(request.URL.Path, remote.StatusCode, started)
		return Response{}, err
	}
	if len(body) > maxProviderBody {
		client.recordMetrics(request.URL.Path, remote.StatusCode, started)
		return Response{}, errors.New("Strava response exceeds size limit")
	}
	response := Response{Body: body, StatusCode: remote.StatusCode, Headers: remote.Header.Clone()}
	client.recordMetrics(request.URL.Path, remote.StatusCode, started)
	if remote.StatusCode < 200 || remote.StatusCode >= 300 {
		return response, HTTPError{StatusCode: remote.StatusCode, RetryAt: retryAt(remote.Header)}
	}
	return response, nil
}

func (client Client) recordMetrics(path string, statusCode int, started time.Time) {
	if client.Metrics != nil {
		client.Metrics.RecordProviderRequest(path, statusCode, time.Since(started))
	}
}

func retryAt(headers http.Header) time.Time {
	if value, err := strconv.Atoi(headers.Get("Retry-After")); err == nil && value > 0 {
		return time.Now().Add(time.Duration(value) * time.Second)
	}
	if value, err := http.ParseTime(headers.Get("Retry-After")); err == nil {
		return value
	}
	return time.Time{}
}

type Activity struct {
	ID        int64  `json:"id"`
	SportType string `json:"sport_type"`
	Type      string `json:"type"`
}

func ParseActivity(data []byte) (Activity, error) {
	var activity Activity
	if err := json.Unmarshal(data, &activity); err != nil {
		return Activity{}, err
	}
	if activity.ID <= 0 {
		return Activity{}, errors.New("Strava activity has no ID")
	}
	return activity, nil
}

func (activity Activity) Cycling() bool {
	kind := activity.SportType
	if kind == "" {
		kind = activity.Type
	}
	switch strings.ToLower(strings.TrimSpace(kind)) {
	case "ride", "virtualride", "mountainbikeride", "gravelride", "ebikeride", "emountainbikeride":
		return true
	default:
		return false
	}
}
