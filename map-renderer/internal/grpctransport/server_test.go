package grpctransport

import (
	"context"
	"errors"
	"net"
	"testing"

	mapsv1 "github.com/ericbutera/bike/map-renderer/gen/bike/maps/v1"
	"github.com/ericbutera/bike/map-renderer/internal/auth"
	"github.com/ericbutera/bike/map-renderer/internal/observability"
	"github.com/ericbutera/bike/map-renderer/internal/render"
	"github.com/stretchr/testify/require"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"
	"google.golang.org/grpc/test/bufconn"
)

type fixtureBrowser struct {
	err error
}

func (b *fixtureBrowser) Render(context.Context, render.Request) ([]byte, error) {
	return []byte("fixture-png"), b.err
}

func fixtureConnection(t *testing.T, browser *fixtureBrowser) *grpc.ClientConn {
	t.Helper()
	renderer := render.New(browser, observability.NewMetrics())
	listener := bufconn.Listen(1024 * 1024)
	server := New(renderer, auth.Token("secret"), observability.NewMetrics())
	finished := make(chan error, 1)
	go func() { finished <- server.Serve(listener) }()
	connection, err := grpc.NewClient("passthrough:///fixture", grpc.WithTransportCredentials(insecure.NewCredentials()),
		grpc.WithContextDialer(func(context.Context, string) (net.Conn, error) { return listener.Dial() }))
	require.NoError(t, err)
	t.Cleanup(func() {
		require.NoError(t, connection.Close())
		server.GracefulStop()
		require.NoError(t, <-finished)
		require.NoError(t, renderer.Close(context.Background()))
	})
	return connection
}

func input() *mapsv1.RenderMapRequest {
	return &mapsv1.RenderMapRequest{Theme: "light", Variant: "full", Dpr: 1,
		Points: []*mapsv1.RoutePoint{{}, {Latitude: 1, Longitude: 1}}}
}

func TestGRPCSnapshotAuthenticationAndValidation(t *testing.T) {
	connection := fixtureConnection(t, &fixtureBrowser{})
	client := mapsv1.NewMapServiceClient(connection)
	ctx := metadata.NewOutgoingContext(t.Context(), metadata.Pairs("authorization", "Bearer secret"))
	_, err := client.Render(t.Context(), input())
	require.Equal(t, codes.Unauthenticated, status.Code(err))
	_, err = client.Render(ctx, &mapsv1.RenderMapRequest{})
	require.Equal(t, codes.InvalidArgument, status.Code(err))
	response, err := client.Render(ctx, input())
	require.NoError(t, err)
	require.Equal(t, []byte("fixture-png"), response.Png)
	require.False(t, response.CacheHit)

}

func TestGRPCSnapshotFailureReturnsAnInternalError(t *testing.T) {
	client := mapsv1.NewMapServiceClient(fixtureConnection(t, &fixtureBrowser{err: errors.New("fixture failure")}))
	ctx := metadata.NewOutgoingContext(t.Context(), metadata.Pairs("authorization", "Bearer secret"))
	_, err := client.Render(ctx, input())
	require.Equal(t, codes.Internal, status.Code(err))
}
