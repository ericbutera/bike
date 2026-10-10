package grpctransport

import (
	"bytes"
	"context"
	"errors"
	"io"
	"log/slog"
	"net"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	mapsv1 "github.com/ericbutera/bike/map-renderer/gen/bike/maps/v1"
	"github.com/ericbutera/bike/map-renderer/internal/auth"
	"github.com/ericbutera/bike/map-renderer/internal/observability"
	"github.com/ericbutera/bike/map-renderer/internal/render"
	"github.com/stretchr/testify/require"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/trace"
	collector "go.opentelemetry.io/proto/otlp/collector/trace/v1"
	tracepb "go.opentelemetry.io/proto/otlp/trace/v1"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/health/grpc_health_v1"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"
	"google.golang.org/grpc/test/bufconn"
	"google.golang.org/protobuf/proto"
)

type fixtureBrowser struct {
	err     error
	context chan trace.SpanContext
}

func (b *fixtureBrowser) Render(ctx context.Context, _ render.Request) ([]byte, error) {
	if b.context != nil {
		b.context <- trace.SpanContextFromContext(ctx)
	}
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

func TestGRPCSnapshotAuthenticationValidationAndHealth(t *testing.T) {
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
	health, err := grpc_health_v1.NewHealthClient(connection).Check(t.Context(), &grpc_health_v1.HealthCheckRequest{Service: "bike.maps.v1.MapService"})
	require.NoError(t, err)
	require.Equal(t, grpc_health_v1.HealthCheckResponse_SERVING, health.Status)
}

func TestGRPCSnapshotFailureRecordsError(t *testing.T) {
	var logs bytes.Buffer
	logger := slog.Default()
	slog.SetDefault(slog.New(slog.NewJSONHandler(&logs, nil)))
	t.Cleanup(func() { slog.SetDefault(logger) })
	client := mapsv1.NewMapServiceClient(fixtureConnection(t, &fixtureBrowser{err: errors.New("fixture failure")}))
	ctx := metadata.NewOutgoingContext(t.Context(), metadata.Pairs("authorization", "Bearer secret"))
	_, err := client.Render(ctx, input())
	require.Equal(t, codes.Internal, status.Code(err))
	require.Contains(t, logs.String(), `"stage":"grpc_render"`)
}

func traceCollector(t *testing.T) (func(context.Context) error, <-chan *collector.ExportTraceServiceRequest) {
	t.Helper()
	exports := make(chan *collector.ExportTraceServiceRequest, 10)
	receiver := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		require.Equal(t, "/v1/traces", r.URL.Path)
		data, err := io.ReadAll(r.Body)
		require.NoError(t, err)
		request := &collector.ExportTraceServiceRequest{}
		require.NoError(t, proto.Unmarshal(data, request))
		exports <- request
		w.Header().Set("Content-Type", "application/x-protobuf")
	}))
	t.Cleanup(receiver.Close)
	provider, propagator, logger := otel.GetTracerProvider(), otel.GetTextMapPropagator(), slog.Default()
	t.Cleanup(func() {
		otel.SetTracerProvider(provider)
		otel.SetTextMapPropagator(propagator)
		slog.SetDefault(logger)
	})
	t.Setenv("OTEL_TRACES_EXPORTER", "otlp")
	t.Setenv("OTEL_EXPORTER_OTLP_ENDPOINT", receiver.URL)
	shutdown, err := observability.InitTracing(t.Context())
	require.NoError(t, err)
	return shutdown, exports
}

func TestGRPCExportsRemoteParentAndRenderChildrenOverOTLP(t *testing.T) {
	shutdown, exports := traceCollector(t)
	browser := &fixtureBrowser{context: make(chan trace.SpanContext, 1)}
	connection := fixtureConnection(t, browser)
	ctx := metadata.NewOutgoingContext(t.Context(), metadata.Pairs("authorization", "Bearer secret",
		"traceparent", "00-00112233445566778899aabbccddeeff-0123456789abcdef-01", "tracestate", "bike=fixture"))
	_, err := mapsv1.NewMapServiceClient(connection).Render(ctx, input())
	require.NoError(t, err)
	serverContext := <-browser.context
	require.Equal(t, "00112233445566778899aabbccddeeff", serverContext.TraceID().String())
	require.Equal(t, "bike=fixture", serverContext.TraceState().String())
	require.NoError(t, shutdown(context.Background()))
	select {
	case request := <-exports:
		assertExportedTree(t, request)
	case <-time.After(3 * time.Second):
		t.Fatal("No OTLP network export received")
	}
}

func assertExportedTree(t *testing.T, request *collector.ExportTraceServiceRequest) {
	t.Helper()
	spans := map[string]*tracepb.Span{}
	for _, resource := range request.ResourceSpans {
		for _, scope := range resource.ScopeSpans {
			for _, span := range scope.Spans {
				spans[span.Name] = span
			}
		}
	}
	server := spans["bike.maps.v1.MapService/Render"]
	require.NotNil(t, server)
	require.Equal(t, tracepb.Span_SPAN_KIND_SERVER, server.Kind)
	require.Equal(t, []byte{1, 35, 69, 103, 137, 171, 205, 239}, server.ParentSpanId)
	queue, snapshot := spans["bike.maps.render_queue_wait"], spans["bike.maps.render"]
	require.NotNil(t, queue)
	require.NotNil(t, snapshot)
	require.Equal(t, server.SpanId, queue.ParentSpanId)
	require.Equal(t, queue.SpanId, snapshot.ParentSpanId)
	require.Equal(t, server.TraceId, snapshot.TraceId)
}
