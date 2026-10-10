package render

import (
	"encoding/json"
	"github.com/stretchr/testify/require"
	"math"
	"os"
	"testing"
)

func fixtureRequest() Request {
	return Request{Theme: "light", Variant: "full", DPR: 1,
		Points: []Point{{Latitude: 44.7631, Longitude: -85.6206}, {Latitude: 44.782, Longitude: -85.576}}}
}

func TestLegacyNormalizedGeometryRemainsValid(t *testing.T) {
	data, err := os.ReadFile("../../testdata/legacy-requests.json")
	require.NoError(t, err)
	var fixtures []struct{ Normalized Request }
	require.NoError(t, json.Unmarshal(data, &fixtures))
	for _, fixture := range fixtures {
		require.NoError(t, fixture.Normalized.Validate())
	}
}

func TestRequestValidationAndDimensions(t *testing.T) {
	request := fixtureRequest()
	for _, variant := range []string{"thumbnail", "full"} {
		request.Variant = variant
		width, height := request.Dimensions()
		if variant == "thumbnail" {
			require.Equal(t, 288, width)
			require.Equal(t, 192, height)
		} else {
			require.Equal(t, 1000, width)
			require.Equal(t, 300, height)
		}
	}
	request.Points = make([]Point, 100000)
	require.NoError(t, request.Validate())
	request.Points = append(request.Points, Point{})
	require.ErrorIs(t, request.Validate(), ErrRequest)
	request.Points = []Point{{}, {}}
	for _, value := range []float64{math.NaN(), math.Inf(1), math.Inf(-1), 91, -91} {
		request.Points[0].Latitude = value
		require.ErrorIs(t, request.Validate(), ErrRequest)
	}
}
