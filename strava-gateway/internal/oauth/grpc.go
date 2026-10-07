package oauth

import (
	"context"
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"errors"
	"strconv"
	"time"

	stravav1 "github.com/ericbutera/bike/strava-gateway/gen/bike/strava/v1"
	"github.com/ericbutera/bike/strava-gateway/internal/storage"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"
)

// GRPCServer exposes the site commands while the provider callbacks stay HTTP.
// Each call carries a short-lived HMAC over the exact method and request fields.
type GRPCServer struct {
	stravav1.UnimplementedGatewayServiceServer
	Handler Handler
}

func (server GRPCServer) authorize(ctx context.Context, method, target string, userID int64, mode string) error {
	site, ok := server.Handler.Sites[target]
	if !ok || site.Secret == "" || userID <= 0 {
		return status.Error(codes.Unauthenticated, "invalid site")
	}
	values, ok := metadata.FromIncomingContext(ctx)
	if !ok || len(values.Get("x-bike-timestamp")) != 1 || len(values.Get("x-bike-signature")) != 1 {
		return status.Error(codes.Unauthenticated, "missing signature")
	}
	timestamp := values.Get("x-bike-timestamp")[0]
	seconds, err := strconv.ParseInt(timestamp, 10, 64)
	if err != nil {
		return status.Error(codes.Unauthenticated, "invalid timestamp")
	}
	now := time.Now()
	if server.Handler.Now != nil {
		now = server.Handler.Now()
	}
	if delta := now.Unix() - seconds; delta > 300 || delta < -300 {
		return status.Error(codes.Unauthenticated, "expired signature")
	}
	provided, err := hex.DecodeString(values.Get("x-bike-signature")[0])
	if err != nil || len(provided) != sha256.Size {
		return status.Error(codes.Unauthenticated, "invalid signature")
	}
	mac := hmac.New(sha256.New, []byte(site.Secret))
	_, _ = mac.Write([]byte(timestamp + "\n" + method + "\n" + target + "\n" + strconv.FormatInt(userID, 10) + "\n" + mode))
	if !hmac.Equal(provided, mac.Sum(nil)) {
		return status.Error(codes.Unauthenticated, "invalid signature")
	}
	return nil
}

func (server GRPCServer) BeginConnect(ctx context.Context, request *stravav1.SiteRequest) (*stravav1.BeginConnectResponse, error) {
	if err := server.authorize(ctx, stravav1.GatewayService_BeginConnect_FullMethodName,
		request.GetTarget(), request.GetSiteUserId(), ""); err != nil {
		return nil, err
	}
	stateBytes := make([]byte, 32)
	if _, err := rand.Read(stateBytes); err != nil {
		return nil, status.Error(codes.Unavailable, "state unavailable")
	}
	state := base64.RawURLEncoding.EncodeToString(stateBytes)
	ctx, cancel := context.WithTimeout(ctx, 3*time.Second)
	defer cancel()
	if err := server.Handler.States.Save(ctx, state, storage.OAuthState{
		Target: request.GetTarget(), UserID: request.GetSiteUserId(),
	}); err != nil {
		return nil, status.Error(codes.Unavailable, "state unavailable")
	}
	return &stravav1.BeginConnectResponse{
		AuthorizationUrl: server.Handler.Provider.AuthorizationURL(server.Handler.CallbackURL, state),
	}, nil
}

func (server GRPCServer) GetConnection(ctx context.Context, request *stravav1.SiteRequest) (*stravav1.ConnectionResponse, error) {
	if err := server.authorize(ctx, stravav1.GatewayService_GetConnection_FullMethodName,
		request.GetTarget(), request.GetSiteUserId(), ""); err != nil {
		return nil, err
	}
	ctx, cancel := context.WithTimeout(ctx, 3*time.Second)
	defer cancel()
	link, err := server.Handler.Connections.FindLinkByUser(ctx, request.GetTarget(), request.GetSiteUserId())
	if errors.Is(err, storage.ErrLinkNotFound) {
		return &stravav1.ConnectionResponse{Configured: true, LastSyncStatus: "never"}, nil
	}
	if err != nil {
		return nil, status.Error(codes.Unavailable, "connection unavailable")
	}
	connection, err := server.Handler.Connections.Find(ctx, link.AthleteID)
	if err != nil {
		return nil, status.Error(codes.Unavailable, "connection unavailable")
	}
	syncStatus, err := server.Handler.Syncs.Status(ctx, link)
	if err != nil {
		return nil, status.Error(codes.Unavailable, "sync status unavailable")
	}
	response := &stravav1.ConnectionResponse{
		Configured: true, Connected: true, AthleteId: &link.AthleteID,
		Scopes: connection.Scopes, LastSyncStatus: syncStatus.Status,
		LastSyncWaitReason: syncStatus.WaitingReason,
	}
	if syncStatus.NextAttemptAt != nil {
		formatted := syncStatus.NextAttemptAt.UTC().Format(time.RFC3339Nano)
		response.LastSyncNextAttemptAt = &formatted
	}
	return response, nil
}

func (server GRPCServer) QueueSync(ctx context.Context, request *stravav1.SyncRequest) (*stravav1.CommandResponse, error) {
	if err := server.authorize(ctx, stravav1.GatewayService_QueueSync_FullMethodName,
		request.GetTarget(), request.GetSiteUserId(), request.GetMode()); err != nil {
		return nil, err
	}
	if request.GetMode() != "initial" && request.GetMode() != "incremental" && request.GetMode() != "full" {
		return nil, status.Error(codes.InvalidArgument, "invalid sync mode")
	}
	ctx, cancel := context.WithTimeout(ctx, 3*time.Second)
	defer cancel()
	link, err := server.Handler.Connections.FindLinkByUser(ctx, request.GetTarget(), request.GetSiteUserId())
	if errors.Is(err, storage.ErrLinkNotFound) {
		return nil, status.Error(codes.NotFound, "Strava is not connected")
	}
	if err != nil || server.Handler.Syncs.Queue(ctx, link, request.GetMode()) != nil {
		return nil, status.Error(codes.Unavailable, "sync unavailable")
	}
	return &stravav1.CommandResponse{Status: "queued"}, nil
}

func (server GRPCServer) Disconnect(ctx context.Context, request *stravav1.SiteRequest) (*stravav1.CommandResponse, error) {
	if err := server.authorize(ctx, stravav1.GatewayService_Disconnect_FullMethodName,
		request.GetTarget(), request.GetSiteUserId(), ""); err != nil {
		return nil, err
	}
	ctx, cancel := context.WithTimeout(ctx, 3*time.Second)
	defer cancel()
	link, err := server.Handler.Connections.FindLinkByUser(ctx, request.GetTarget(), request.GetSiteUserId())
	if errors.Is(err, storage.ErrLinkNotFound) {
		return &stravav1.CommandResponse{Status: "disconnected"}, nil
	}
	if err != nil || server.Handler.Connections.DisableLink(ctx, link) != nil {
		return nil, status.Error(codes.Unavailable, "disconnect unavailable")
	}
	return &stravav1.CommandResponse{Status: "disconnected"}, nil
}
