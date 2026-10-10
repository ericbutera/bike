package render

import (
	"errors"
	"math"

	"github.com/go-playground/validator/v10"
	"github.com/samber/lo"
)

var (
	ErrRequest = errors.New("invalid map request")
	validation = validator.New()
)

type Point struct {
	Latitude  float64 `json:"latitude" validate:"gte=-90,lte=90"`
	Longitude float64 `json:"longitude" validate:"gte=-180,lte=180"`
}

type Request struct {
	Theme   string  `json:"theme" validate:"oneof=light dark"`
	Variant string  `json:"variant" validate:"oneof=thumbnail full"`
	DPR     int     `json:"dpr" validate:"oneof=1 2"`
	Points  []Point `json:"points" validate:"min=2,max=100000,dive"`
}

func (r Request) Validate() error {
	if err := validation.Struct(r); err != nil {
		return ErrRequest
	}
	if lo.ContainsBy(r.Points, func(p Point) bool {
		return !finite(p.Latitude) || !finite(p.Longitude)
	}) {
		return ErrRequest
	}
	return nil
}

func finite(n float64) bool { return !math.IsNaN(n) && !math.IsInf(n, 0) }

func (r Request) Dimensions() (int, int) {
	if r.Variant == "thumbnail" {
		return 288, 192
	}
	return 1000, 300
}
