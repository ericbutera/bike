package app

import (
	"context"
	"errors"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/health/grpc_health_v1"
)

func CheckHealth(ctx context.Context, config Config) (err error) {
	connection, err := grpc.NewClient(config.GRPCAddress, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return err
	}
	defer func() { err = errors.Join(err, connection.Close()) }()
	ctx, cancel := context.WithTimeout(ctx, 2*time.Second)
	defer cancel()
	response, err := grpc_health_v1.NewHealthClient(connection).Check(ctx, &grpc_health_v1.HealthCheckRequest{Service: "bike.maps.v1.MapService"})
	if err != nil {
		return err
	}
	if response.GetStatus() != grpc_health_v1.HealthCheckResponse_SERVING {
		return errors.New("snapshot worker is not serving")
	}
	return nil
}
