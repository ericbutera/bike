package app

import (
	"errors"
	"net"

	"github.com/caarlos0/env/v11"
	"github.com/go-playground/validator/v10"
)

type Config struct {
	AssetsAddress    string `env:"MAP_ASSETS_ADDRESS" envDefault:"127.0.0.1:3100"`
	GRPCAddress      string `env:"MAP_GRPC_ADDRESS" envDefault:":50051"`
	MetricsPort      int    `env:"METRICS_PORT" envDefault:"9090" validate:"min=1,max=65535"`
	Assets           string `env:"MAP_ASSETS_DIR" envDefault:"/app"`
	Token            string `env:"MAP_SERVICE_TOKEN"`
	ChromeExecutable string `env:"CHROME_EXECUTABLE"`
}

func LoadConfig() (Config, error) {
	config, err := env.ParseAs[Config]()
	if err != nil {
		return config, err
	}
	if err := validator.New().Struct(config); err != nil {
		return config, err
	}
	_, err = config.browserURL()
	return config, err
}

func (c Config) browserURL() (string, error) {
	host, port, err := net.SplitHostPort(c.AssetsAddress)
	if err != nil {
		return "", err
	}
	if ip := net.ParseIP(host); ip == nil || !ip.IsLoopback() {
		return "", errors.New("MAP_ASSETS_ADDRESS must bind a loopback IP")
	}
	if port == "0" {
		return "", errors.New("MAP_ASSETS_ADDRESS must specify a fixed browser port")
	}
	return "http://" + net.JoinHostPort(host, port) + "/", nil
}
