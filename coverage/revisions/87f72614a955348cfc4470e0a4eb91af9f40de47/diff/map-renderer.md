# Diff Coverage
## Diff: 0c20debc99a0c5b6ef202e41ba83e03ae7127e65...HEAD, staged, unstaged and untracked changes

- map-renderer/cmd/renderer/main&#46;go (64.7%): Missing lines 15-20
- map-renderer/internal/app/app&#46;go (94.8%): Missing lines 74,126-127,134
- map-renderer/internal/app/config&#46;go (100%)
- map-renderer/internal/app/health&#46;go (86.7%): Missing lines 16-17
- map-renderer/internal/auth/token&#46;go (100%)
- map-renderer/internal/browser/browser&#46;go (28.4%): Missing lines 39-45,57,61-65,68-94,98-112,114-117,121-129
- map-renderer/internal/grpctransport/server&#46;go (100%)
- map-renderer/internal/httptransport/server&#46;go (100%)
- map-renderer/internal/observability/metrics&#46;go (100%)
- map-renderer/internal/observability/tracing&#46;go (95.1%): Missing lines 50-51
- map-renderer/internal/render/renderer&#46;go (100%)
- map-renderer/internal/render/request&#46;go (85.7%): Missing lines 35-36

## Summary

- **Total**: 431 lines
- **Missing**: 84 lines
- **Coverage**: 80.51%



## map-renderer/cmd/renderer/main&#46;go

```
  11 	"github.com/ericbutera/bike/map-renderer/internal/app"
  12 )
  13 
  14 func main() {
! 15 	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGTERM, syscall.SIGINT)
! 16 	defer stop()
! 17 	if err := run(ctx, os.Args[1:]); err != nil {
! 18 		slog.Error("Map renderer failed", "error", err)
! 19 		os.Exit(1)
! 20 	}
  21 }
  22 
  23 func run(ctx context.Context, args []string) error {
  24 	config, err := app.LoadConfig()
```


---



## map-renderer/internal/app/app&#46;go

```
  70 		return err
  71 	}
  72 	select {
  73 	case <-ctx.Done():
! 74 	case err = <-servers.errors:
  75 	}
  76 	return err
  77 }
```


---


```
  122 	for _, server := range []*http.Server{s.http, s.metrics} {
  123 		go func() {
  124 			err := server.Shutdown(ctx)
  125 			if err != nil {
! 126 				err = errors.Join(err, server.Close())
! 127 			}
  128 			results <- err
  129 		}()
  130 	}
  131 	select {
```


---


```
  130 	}
  131 	select {
  132 	case <-done:
  133 	case <-ctx.Done():
! 134 		s.grpc.Stop()
  135 	}
  136 	return errors.Join(<-results, <-results, ctx.Err())
  137 }
```


---



## map-renderer/internal/app/health&#46;go

```
  12 
  13 func CheckHealth(ctx context.Context, config Config) (err error) {
  14 	connection, err := grpc.NewClient(config.GRPCAddress, grpc.WithTransportCredentials(insecure.NewCredentials()))
  15 	if err != nil {
! 16 		return err
! 17 	}
  18 	defer func() { err = errors.Join(err, connection.Close()) }()
  19 	ctx, cancel := context.WithTimeout(ctx, 2*time.Second)
  20 	defer cancel()
  21 	response, err := grpc_health_v1.NewHealthClient(connection).Check(ctx, &grpc_health_v1.HealthCheckRequest{Service: "bike.maps.v1.MapService"})
```


---



## map-renderer/internal/browser/browser&#46;go

```
  35 	options := append([]chromedp.ExecAllocatorOption{}, chromedp.DefaultExecAllocatorOptions[:]...)
  36 	options = append(options, chromedp.NoSandbox, chromedp.Flag("use-gl", "angle"),
  37 		chromedp.Flag("use-angle", "swiftshader"), chromedp.Flag("enable-unsafe-swiftshader", true))
  38 	if executable == "" {
! 39 		matches, err := filepath.Glob("/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-linux*/chrome-headless-shell")
! 40 		if err != nil {
! 41 			return nil, err
! 42 		}
! 43 		if len(matches) > 0 {
! 44 			executable = matches[0]
! 45 		}
  46 	}
  47 	if executable != "" {
  48 		options = append(options, chromedp.ExecPath(executable))
  49 	}
```


---


```
   53 		cancel()
   54 		stop()
   55 		return nil, err
   56 	}
!  57 	return &Browser{ctx: browserCtx, cancel: cancel, stopAllocator: stop, url: url}, nil
   58 }
   59 
   60 func (b *Browser) Close() error {
!  61 	err := chromedp.Cancel(b.ctx)
!  62 	b.cancel()
!  63 	b.stopAllocator()
!  64 	return err
!  65 }
   66 
   67 func (b *Browser) Render(ctx context.Context, request render.Request) (png []byte, err error) {
!  68 	tab, closeTab := chromedp.NewContext(b.ctx, chromedp.WithNewBrowserContext())
!  69 	defer closeTab()
!  70 	tab, stop := context.WithTimeout(tab, 45*time.Second)
!  71 	defer stop()
!  72 	if err := chromedp.Do(tab, chromedp.Func(func(ctx context.Context, target *chromedp.Target) error {
!  73 		_, err := cdp.Call(ctx, target, log.Enable, cdp.Empty{})
!  74 		return err
!  75 	})); err != nil {
!  76 		return nil, err
!  77 	}
!  78 	diagnostics, stopConsole := watchConsole(tab)
!  79 	defer stopConsole()
!  80 	if err := b.renderMap(tab, ctx, request); err != nil {
!  81 		return nil, err
!  82 	}
!  83 	if err := diagnostics.err(); err != nil {
!  84 		return nil, err
!  85 	}
!  86 	spanCtx, span := otel.Tracer("bike-map-renderer").Start(ctx, "bike.maps.screenshot")
!  87 	defer span.End()
!  88 	png, err = chromedp.Run(tab, chromedp.CaptureScreenshot())
!  89 	stopConsole()
!  90 	err = errors.Join(err, diagnostics.err())
!  91 	if err != nil {
!  92 		observability.Error(spanCtx, "Map screenshot failed", err, "screenshot")
!  93 	}
!  94 	return png, err
   95 }
   96 
   97 func (b *Browser) renderMap(tab, parent context.Context, request render.Request) error {
!  98 	ctx, span := otel.Tracer("bike-map-renderer").Start(parent, "bike.maps.browser_render")
!  99 	defer span.End()
! 100 	width, height := request.Dimensions()
! 101 	data, err := json.Marshal(request)
! 102 	if err != nil {
! 103 		return err
! 104 	}
! 105 	err = chromedp.Do(tab, chromedp.EmulateViewport(int64(width), int64(height), chromedp.EmulateScale(float64(request.DPR))), chromedp.Navigate(b.url))
! 106 	if err == nil {
! 107 		_, err = chromedp.Run(tab, chromedp.Poll[bool](`typeof window.renderMap === "function"`))
! 108 	}
! 109 	if err == nil {
! 110 		_, err = chromedp.Run(tab, chromedp.Evaluate[bool]("window.renderMap("+string(data)+")", func(p *runtime.EvaluateParams) {
! 111 			p.AwaitPromise = lo.ToPtr(true)
! 112 		}))
  113 	}
! 114 	if err != nil {
! 115 		observability.Error(ctx, "Map browser rendering failed", err, "browser_render")
! 116 	}
! 117 	return err
  118 }
  119 
  120 func watchConsole(ctx context.Context) (*diagnostics, func()) {
! 121 	ctx, cancel := context.WithCancel(ctx)
! 122 	messages := chromedp.Console(ctx)
! 123 	d := &diagnostics{}
! 124 	done := make(chan struct{})
! 125 	go func() {
! 126 		defer close(done)
! 127 		d.collect(messages)
! 128 	}()
! 129 	return d, func() { cancel(); <-done }
  130 }
  131 
  132 func (d *diagnostics) collect(messages iter.Seq2[chromedp.ConsoleMessage, error]) {
  133 	for message, err := range messages {
```


---



## map-renderer/internal/observability/tracing&#46;go

```
  46 	}
  47 	if config.exportsOTLP() {
  48 		exporter, err := otlptracehttp.New(ctx)
  49 		if err != nil {
! 50 			return nil, err
! 51 		}
  52 		options = append(options, sdktrace.WithBatcher(exporter))
  53 	}
  54 	provider := sdktrace.NewTracerProvider(options...)
  55 	otel.SetTracerProvider(provider)
```


---



## map-renderer/internal/render/request&#46;go

```
  31 	}
  32 	if lo.ContainsBy(r.Points, func(p Point) bool {
  33 		return !finite(p.Latitude) || !finite(p.Longitude)
  34 	}) {
! 35 		return ErrRequest
! 36 	}
  37 	return nil
  38 }
  39 
  40 func finite(n float64) bool { return !math.IsNaN(n) && !math.IsInf(n, 0) }
```


---


