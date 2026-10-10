package grpctransport

import (
	"context"
	"time"

	mapsv1 "github.com/ericbutera/bike/map-renderer/gen/bike/maps/v1"
	"github.com/ericbutera/bike/map-renderer/internal/auth"
	"github.com/ericbutera/bike/map-renderer/internal/observability"
	"github.com/ericbutera/bike/map-renderer/internal/render"
	"github.com/samber/lo"
	"go.opentelemetry.io/contrib/instrumentation/google.golang.org/grpc/otelgrpc"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/health"
	"google.golang.org/grpc/health/grpc_health_v1"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"
)

type Server struct {
	mapsv1.UnimplementedMapServiceServer
	renderer *render.Renderer
	token    auth.Token
	metrics  *observability.Metrics
}

func New(renderer *render.Renderer, token auth.Token, metrics *observability.Metrics) *grpc.Server {
	server := grpc.NewServer(grpc.MaxRecvMsgSize(10_000_000), grpc.MaxSendMsgSize(20_000_000),
		grpc.StatsHandler(otelgrpc.NewServerHandler()))
	mapsv1.RegisterMapServiceServer(server, &Server{renderer: renderer, token: token, metrics: metrics})
	readiness := health.NewServer()
	readiness.SetServingStatus("bike.maps.v1.MapService", grpc_health_v1.HealthCheckResponse_SERVING)
	grpc_health_v1.RegisterHealthServer(server, readiness)
	return server
}

func (s *Server) Render(ctx context.Context, input *mapsv1.RenderMapRequest) (response *mapsv1.RenderMapResponse, err error) {
	started := time.Now()
	defer func() { s.metrics.GRPC(int(status.Code(err)), started) }()
	md, _ := metadata.FromIncomingContext(ctx)
	if !s.token.Authorized(first(md.Get("authorization"))) {
		return nil, status.Error(codes.Unauthenticated, "Unauthorized")
	}
	request := decode(input)
	if err := request.Validate(); err != nil {
		return nil, status.Error(codes.InvalidArgument, "Invalid map request")
	}
	result, err := s.renderer.Render(ctx, request)
	if err != nil {
		observability.Error(ctx, "gRPC map rendering failed", err, "grpc_render")
		return nil, status.Error(codes.Internal, "Map rendering failed")
	}
	return &mapsv1.RenderMapResponse{Png: result.PNG}, nil
}

func first(values []string) string {
	if len(values) == 0 {
		return ""
	}
	return values[0]
}

func decode(input *mapsv1.RenderMapRequest) render.Request {
	return render.Request{Theme: input.GetTheme(), Variant: input.GetVariant(), DPR: int(input.GetDpr()),
		Points: lo.Map(input.GetPoints(), func(p *mapsv1.RoutePoint, _ int) render.Point {
			return render.Point{Latitude: p.GetLatitude(), Longitude: p.GetLongitude()}
		})}
}
