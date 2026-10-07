package oauth

import (
	"context"
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"strconv"
	"testing"
	"time"

	stravav1 "github.com/ericbutera/bike/strava-gateway/gen/bike/strava/v1"
	"google.golang.org/grpc/metadata"
)

func TestGRPCSiteSignatureBindsMethodAndUser(t *testing.T) {
	now := time.Unix(1_800_000_000, 0)
	server := GRPCServer{Handler: Handler{Sites: map[string]Site{"rust": {Secret: "shared-secret"}}, Now: func() time.Time { return now }}}
	method := stravav1.GatewayService_GetConnection_FullMethodName
	timestamp := strconv.FormatInt(now.Unix(), 10)
	mac := hmac.New(sha256.New, []byte("shared-secret"))
	_, _ = mac.Write([]byte(timestamp + "\n" + method + "\nrust\n7\n"))
	ctx := metadata.NewIncomingContext(context.Background(), metadata.Pairs(
		"x-bike-timestamp", timestamp, "x-bike-signature", hex.EncodeToString(mac.Sum(nil))))
	if err := server.authorize(ctx, method, "rust", 7, ""); err != nil {
		t.Fatalf("valid signature rejected: %v", err)
	}
	for _, changed := range []struct {
		method string
		userID int64
		target string
	}{
		{method: stravav1.GatewayService_Disconnect_FullMethodName, userID: 7, target: "rust"},
		{method: method, userID: 8, target: "rust"},
		{method: method, userID: 7, target: "retired"},
	} {
		if err := server.authorize(ctx, changed.method, changed.target, changed.userID, ""); err == nil {
			t.Fatalf("accepted modified request: %+v", changed)
		}
	}
}
