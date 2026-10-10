package app

import (
	"github.com/stretchr/testify/require"
	"testing"
)

func TestConfigDefaultsAndOverrides(t *testing.T) {
	t.Setenv("METRICS_PORT", "9090")
	config, err := LoadConfig()
	require.NoError(t, err)
	url, err := config.browserURL()
	require.NoError(t, err)
	require.Equal(t, "http://127.0.0.1:3100/", url)
	t.Setenv("MAP_ASSETS_ADDRESS", "127.0.0.1:3110")
	config, err = LoadConfig()
	require.NoError(t, err)
	url, err = config.browserURL()
	require.NoError(t, err)
	require.Equal(t, "http://127.0.0.1:3110/", url)
}

func TestConfigRejectsInvalidPorts(t *testing.T) {
	for _, test := range []struct{ name, value string }{
		{"METRICS_PORT", "0"}, {"METRICS_PORT", "65536"}, {"METRICS_PORT", "abc"},
	} {
		t.Run(test.name+test.value, func(t *testing.T) { t.Setenv(test.name, test.value); _, err := LoadConfig(); require.Error(t, err) })
	}
	for _, address := range []string{"invalid", "127.0.0.1:0", ":3100", "0.0.0.0:3100", "192.168.1.10:3100"} {
		_, err := (Config{AssetsAddress: address}).browserURL()
		require.Error(t, err)
	}
}
