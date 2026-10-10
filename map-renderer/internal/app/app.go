package app

import (
	"context"
	"errors"
	"fmt"
	"net"
	"net/http"
	"time"

	"github.com/ericbutera/bike/map-renderer/internal/auth"
	"github.com/ericbutera/bike/map-renderer/internal/browser"
	"github.com/ericbutera/bike/map-renderer/internal/grpctransport"
	"github.com/ericbutera/bike/map-renderer/internal/httptransport"
	"github.com/ericbutera/bike/map-renderer/internal/observability"
	"github.com/ericbutera/bike/map-renderer/internal/render"
	"google.golang.org/grpc"
)

type servers struct {
	http        *http.Server
	metrics     *http.Server
	grpc        *grpc.Server
	grpcAddress string
	errors      chan error
}

type browserRuntime interface {
	render.Browser
	Close() error
}
type browserFactory func(context.Context, Config) (browserRuntime, error)

func Run(ctx context.Context, config Config) error {
	return run(ctx, config, openBrowser)
}

func openBrowser(ctx context.Context, config Config) (browserRuntime, error) {
	url, err := config.browserURL()
	if err != nil {
		return nil, err
	}
	return browser.New(ctx, config.ChromeExecutable, url)
}

func run(ctx context.Context, config Config, openBrowser browserFactory) (err error) {
	stopTracing, err := observability.InitTracing(context.Background())
	if err != nil {
		return err
	}
	defer func() {
		shutdown, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		err = errors.Join(err, stopTracing(shutdown))
	}()
	browser, err := openBrowser(context.Background(), config)
	if err != nil {
		return err
	}
	defer func() { err = errors.Join(err, browser.Close()) }()
	metrics := observability.NewMetrics()
	renderer := render.New(browser, metrics)
	servers := newServers(config, renderer, metrics)
	defer func() {
		shutdown, cancel := context.WithTimeout(context.Background(), 60*time.Second)
		defer cancel()
		err = errors.Join(err, servers.shutdown(shutdown), renderer.Close(shutdown))
	}()
	if err := servers.start(); err != nil {
		return err
	}
	select {
	case <-ctx.Done():
	case err = <-servers.errors:
	}
	return err
}

func newServers(config Config, renderer *render.Renderer, metrics *observability.Metrics) *servers {
	token := auth.Token(config.Token)
	return &servers{
		http:    &http.Server{Addr: config.AssetsAddress, Handler: httptransport.AssetsHandler(config.Assets), ReadHeaderTimeout: 10 * time.Second},
		metrics: &http.Server{Addr: fmt.Sprintf(":%d", config.MetricsPort), Handler: httptransport.MetricsHandler(metrics), ReadHeaderTimeout: 10 * time.Second},
		grpc:    grpctransport.New(renderer, token, metrics), grpcAddress: config.GRPCAddress, errors: make(chan error, 3),
	}
}

func (s *servers) start() (err error) {
	var listeners []net.Listener
	defer func() {
		if err != nil {
			for _, listener := range listeners {
				err = errors.Join(err, listener.Close())
			}
		}
	}()
	for _, address := range []string{s.grpcAddress, s.http.Addr, s.metrics.Addr} {
		listener, err := net.Listen("tcp", address)
		if err != nil {
			return err
		}
		listeners = append(listeners, listener)
	}
	go func() { s.errors <- s.grpc.Serve(listeners[0]) }()
	go s.serveHTTP(s.http, listeners[1])
	go s.serveHTTP(s.metrics, listeners[2])
	return nil
}

func (s *servers) serveHTTP(server *http.Server, listener net.Listener) {
	err := server.Serve(listener)
	if errors.Is(err, http.ErrServerClosed) {
		err = nil
	}
	s.errors <- err
}

func (s *servers) shutdown(ctx context.Context) error {
	done := make(chan struct{})
	go func() { s.grpc.GracefulStop(); close(done) }()
	results := make(chan error, 2)
	for _, server := range []*http.Server{s.http, s.metrics} {
		go func() {
			err := server.Shutdown(ctx)
			if err != nil {
				err = errors.Join(err, server.Close())
			}
			results <- err
		}()
	}
	select {
	case <-done:
	case <-ctx.Done():
		s.grpc.Stop()
	}
	return errors.Join(<-results, <-results, ctx.Err())
}
