package provider

import "context"

// OAuthAPI is the provider seam used by account linking. Implementations may be
// the HTTP adapter or a fixture-backed fake; application code never needs an SDK.
type OAuthAPI interface {
	AuthorizationURL(redirectURL, state string) string
	ExchangeCode(context.Context, string) (Token, Response, error)
	AuthenticatedAthlete(context.Context, string) (int64, Response, error)
}

// SyncAPI is the provider seam used by the single sync worker.
type SyncAPI interface {
	Refresh(context.Context, string) (Token, Response, error)
	ListActivities(context.Context, string, int, int64) ([]int64, Response, error)
	FetchActivity(context.Context, string, int64) (Response, error)
	FetchStreams(context.Context, string, int64) (Response, error)
}

var _ OAuthAPI = Client{}
var _ SyncAPI = Client{}
