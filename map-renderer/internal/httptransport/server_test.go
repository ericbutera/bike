package httptransport

import (
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"

	"github.com/ericbutera/bike/map-renderer/internal/observability"
	"github.com/stretchr/testify/require"
)

func TestChromiumAssetsAreExplicitAndRenderingIsNotHTTP(t *testing.T) {
	assets := t.TempDir()
	for _, path := range []string{"index.html", "styles/fiord-v1.json", "vendor/maplibre-gl.css", "private.txt"} {
		require.NoError(t, os.MkdirAll(filepath.Join(assets, filepath.Dir(path)), 0700))
		require.NoError(t, os.WriteFile(filepath.Join(assets, path), []byte("fixture asset"), 0600))
	}
	handler := AssetsHandler(assets)
	for _, test := range []struct {
		method, path string
		status       int
	}{
		{"GET", "/", 200}, {"GET", "/styles/fiord-v1.json", 200}, {"GET", "/vendor/maplibre-gl.css", 200},
		{"GET", "/favicon.ico", 204}, {"GET", "/private.txt", 404}, {"GET", "/vendor/private.mjs", 404},
		{"GET", "/healthz", 404}, {"GET", "/metrics", 404}, {"POST", "/render", 404},
	} {
		response := httptest.NewRecorder()
		handler.ServeHTTP(response, httptest.NewRequest(test.method, test.path, nil))
		require.Equal(t, test.status, response.Code, test.path)
		if test.status == 200 {
			require.Equal(t, "fixture asset", response.Body.String())
		}
	}
	metrics := MetricsHandler(observability.NewMetrics())
	response := httptest.NewRecorder()
	metrics.ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/metrics", nil))
	require.Equal(t, 200, response.Code)
	require.Contains(t, response.Body.String(), "bike_maps_render_queue_depth")
}
