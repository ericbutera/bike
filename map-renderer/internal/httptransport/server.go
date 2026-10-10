// Package httptransport serves Chromium's private assets and Prometheus metrics.
package httptransport

import (
	"net/http"
	"os"

	"github.com/ericbutera/bike/map-renderer/internal/observability"
	"github.com/prometheus/client_golang/prometheus/promhttp"
)

func AssetsHandler(assets string) http.Handler {
	mux := http.NewServeMux()
	files := map[string]string{"/{$}": "index.html",
		"/styles/route-light-v1.json": "styles/route-light-v1.json", "/styles/fiord-v1.json": "styles/fiord-v1.json"}
	for _, name := range []string{"maplibre-gl.css", "maplibre-gl.mjs", "maplibre-gl-shared.mjs", "maplibre-gl-worker.mjs"} {
		files["/vendor/"+name] = "vendor/" + name
	}
	for route, path := range files {
		mux.HandleFunc("GET "+route, func(w http.ResponseWriter, r *http.Request) {
			http.ServeFileFS(w, r, os.DirFS(assets), path)
		})
	}
	mux.HandleFunc("GET /favicon.ico", func(w http.ResponseWriter, _ *http.Request) { w.WriteHeader(http.StatusNoContent) })
	return mux
}

func MetricsHandler(metrics *observability.Metrics) http.Handler {
	mux := http.NewServeMux()
	mux.Handle("GET /metrics", promhttp.HandlerFor(metrics.Registry, promhttp.HandlerOpts{}))
	return mux
}
