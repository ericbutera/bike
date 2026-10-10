package browser

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"iter"
	"path/filepath"
	"sync"
	"time"

	"github.com/chromedp/cdproto/cdp"
	"github.com/chromedp/cdproto/log"
	"github.com/chromedp/cdproto/runtime"
	"github.com/chromedp/chromedp"
	"github.com/ericbutera/bike/map-renderer/internal/observability"
	"github.com/ericbutera/bike/map-renderer/internal/render"
	"github.com/samber/lo"
	"go.opentelemetry.io/otel"
)

type Browser struct {
	ctx           context.Context
	cancel        context.CancelFunc
	stopAllocator context.CancelFunc
	url           string
}
type diagnostics struct {
	mu     sync.Mutex
	errors []error
}

func New(ctx context.Context, executable, url string) (*Browser, error) {
	options := append([]chromedp.ExecAllocatorOption{}, chromedp.DefaultExecAllocatorOptions[:]...)
	options = append(options, chromedp.NoSandbox, chromedp.Flag("use-gl", "angle"),
		chromedp.Flag("use-angle", "swiftshader"), chromedp.Flag("enable-unsafe-swiftshader", true))
	if executable == "" {
		matches, err := filepath.Glob("/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-linux*/chrome-headless-shell")
		if err != nil {
			return nil, err
		}
		if len(matches) > 0 {
			executable = matches[0]
		}
	}
	if executable != "" {
		options = append(options, chromedp.ExecPath(executable))
	}
	allocator, stop := chromedp.NewExecAllocator(ctx, options...)
	browserCtx, cancel := chromedp.NewContext(allocator)
	if err := chromedp.Do(browserCtx); err != nil {
		cancel()
		stop()
		return nil, err
	}
	return &Browser{ctx: browserCtx, cancel: cancel, stopAllocator: stop, url: url}, nil
}

func (b *Browser) Close() error {
	err := chromedp.Cancel(b.ctx)
	b.cancel()
	b.stopAllocator()
	return err
}

func (b *Browser) Render(ctx context.Context, request render.Request) (png []byte, err error) {
	tab, closeTab := chromedp.NewContext(b.ctx, chromedp.WithNewBrowserContext())
	defer closeTab()
	tab, stop := context.WithTimeout(tab, 45*time.Second)
	defer stop()
	if err := chromedp.Do(tab, chromedp.Func(func(ctx context.Context, target *chromedp.Target) error {
		_, err := cdp.Call(ctx, target, log.Enable, cdp.Empty{})
		return err
	})); err != nil {
		return nil, err
	}
	diagnostics, stopConsole := watchConsole(tab)
	defer stopConsole()
	if err := b.renderMap(tab, ctx, request); err != nil {
		return nil, err
	}
	if err := diagnostics.err(); err != nil {
		return nil, err
	}
	spanCtx, span := otel.Tracer("bike-map-renderer").Start(ctx, "bike.maps.screenshot")
	defer span.End()
	png, err = chromedp.Run(tab, chromedp.CaptureScreenshot())
	stopConsole()
	err = errors.Join(err, diagnostics.err())
	if err != nil {
		observability.Error(spanCtx, "Map screenshot failed", err, "screenshot")
	}
	return png, err
}

func (b *Browser) renderMap(tab, parent context.Context, request render.Request) error {
	ctx, span := otel.Tracer("bike-map-renderer").Start(parent, "bike.maps.browser_render")
	defer span.End()
	width, height := request.Dimensions()
	data, err := json.Marshal(request)
	if err != nil {
		return err
	}
	err = chromedp.Do(tab, chromedp.EmulateViewport(int64(width), int64(height), chromedp.EmulateScale(float64(request.DPR))), chromedp.Navigate(b.url))
	if err == nil {
		_, err = chromedp.Run(tab, chromedp.Poll[bool](`typeof window.renderMap === "function"`))
	}
	if err == nil {
		_, err = chromedp.Run(tab, chromedp.Evaluate[bool]("window.renderMap("+string(data)+")", func(p *runtime.EvaluateParams) {
			p.AwaitPromise = lo.ToPtr(true)
		}))
	}
	if err != nil {
		observability.Error(ctx, "Map browser rendering failed", err, "browser_render")
	}
	return err
}

func watchConsole(ctx context.Context) (*diagnostics, func()) {
	ctx, cancel := context.WithCancel(ctx)
	messages := chromedp.Console(ctx)
	d := &diagnostics{}
	done := make(chan struct{})
	go func() {
		defer close(done)
		d.collect(messages)
	}()
	return d, func() { cancel(); <-done }
}

func (d *diagnostics) collect(messages iter.Seq2[chromedp.ConsoleMessage, error]) {
	for message, err := range messages {
		if errors.Is(err, context.Canceled) {
			return
		}
		if message.Type == chromedp.ConsoleWarning || message.Type == chromedp.ConsoleError || message.IsException() {
			err = fmt.Errorf("browser console %s", message.String())
		}
		if err != nil {
			d.mu.Lock()
			d.errors = append(d.errors, err)
			d.mu.Unlock()
		}
	}
}

func (d *diagnostics) err() error { d.mu.Lock(); defer d.mu.Unlock(); return errors.Join(d.errors...) }
