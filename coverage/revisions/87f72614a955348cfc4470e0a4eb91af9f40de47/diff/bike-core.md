# Diff Coverage
## Diff: 0c20debc99a0c5b6ef202e41ba83e03ae7127e65...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_maps/cache&#46;rs (96.5%): Missing lines 52,114-115,142
- bike-rs/bike-core/src/activity_maps/client&#46;rs (98.6%): Missing lines 99
- bike-rs/bike-core/src/activity_maps/metrics&#46;rs (100%)
- bike-rs/bike-core/src/activity_maps/service&#46;rs (85.7%): Missing lines 66,78-79,82,109-110,112-114,116
- bike-rs/bike-core/src/activity_maps/types&#46;rs (97.0%): Missing lines 77,80
- bike-rs/bike-core/src/config&#46;rs (100%)
- bike-rs/bike-core/src/observability&#46;rs (80.0%): Missing lines 45-46,113-114,117,120-121,162-165
- bike-rs/bike-core/src/strava&#46;rs (100%)

## Summary

- **Total**: 413 lines
- **Missing**: 28 lines
- **Coverage**: 93.22%



## bike-rs/bike-core/src/activity_maps/cache&#46;rs

```
  48         let path = self.root.join(format!("{key}.png"));
  49         let metadata = match fs::metadata(&path) {
  50             Ok(metadata) => metadata,
  51             Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
! 52             Err(error) => return Err(error),
  53         };
  54         if self.expired(metadata.modified()?) {
  55             remove_missing_ok(&path)?;
  56             return Ok(None);
```


---


```
  110             let entry = entry?;
  111             let name = entry.file_name().to_string_lossy().into_owned();
  112             let metadata = match entry.metadata() {
  113                 Ok(metadata) => metadata,
! 114                 Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
! 115                 Err(error) => return Err(error),
  116             };
  117             if !metadata.is_file() {
  118                 continue;
  119             }
```


---


```
  138 }
  139 
  140 fn owned_file(name: &str) -> bool {
  141     let Some((key, suffix)) = name.split_once(".png") else {
! 142         return false;
  143     };
  144     key.len() == 64
  145         && key.bytes().all(|b| b.is_ascii_hexdigit())
  146         && (suffix.is_empty() || suffix.starts_with(".tmp-"))
```


---



## bike-rs/bike-core/src/activity_maps/client&#46;rs

```
   95             }
   96             .into(),
   97             variant: match self.variant {
   98                 Variant::Full => "full",
!  99                 Variant::Thumbnail => "thumbnail",
  100             }
  101             .into(),
  102             dpr: self.dpr.into(),
  103             points: self
```


---



## bike-rs/bike-core/src/activity_maps/service&#46;rs

```
  62             return Ok(cached(png));
  63         }
  64         let _guard = self.pending.lock().await;
  65         if let Some(png) = self.cache.read(key.clone()).await? {
! 66             return Ok(cached(png));
  67         }
  68         METRICS.misses.inc();
  69         let timer = METRICS.duration.with_label_values(&["miss"]).start_timer();
  70         let png = self.client.snapshot(request).await?;
```


---


```
  74     }
  75 
  76     async fn store_image(&self, key: String, png: &[u8]) {
  77         if let Err(error) = self.cache.write(key, png.to_vec()).await {
! 78             METRICS.write_failures.inc();
! 79             tracing::error!(%error, stage = "cache_write", "Map image cache write failed");
  80         }
  81         if let Err(error) = self.prune().await {
! 82             tracing::error!(%error, stage = "cache_stats", "Map cache statistics refresh failed");
  83         }
  84     }
  85 
  86     pub async fn prune(&self) -> Result<(), MapImageError> {
```


---


```
  105 async fn maintain(service: Weak<MapImageService>) {
  106     let mut interval = tokio::time::interval(Duration::from_secs(3600));
  107     loop {
  108         interval.tick().await;
! 109         let Some(service) = service.upgrade() else {
! 110             return;
  111         };
! 112         if let Err(error) = service.prune().await {
! 113             tracing::error!(%error, stage = "cache_cleanup", "Map cache cleanup failed");
! 114         }
  115     }
! 116 }
```


---



## bike-rs/bike-core/src/activity_maps/types&#46;rs

```
  73             })
  74             .filter(Point::valid)
  75             .collect();
  76         if points.len() < 2 {
! 77             return Err(MapImageError::NotFound);
  78         }
  79         if points.len() > 100_000 {
! 80             return Err(MapImageError::Invalid);
  81         }
  82         Ok(Self {
  83             theme: options.theme,
  84             variant,
```


---



## bike-rs/bike-core/src/observability&#46;rs

```
  41 pub fn init_observability(service_name: &'static str) -> ObservabilityGuard {
  42     global::set_text_map_propagator(TraceContextPropagator::new());
  43 
  44     match trace_exporter_config() {
! 45         TraceExporterConfig::SdkDisabled => init_logs_only(service_name, "OTEL_SDK_DISABLED=true"),
! 46         TraceExporterConfig::Disabled { reason } => init_without_exporter(service_name, reason),
  47         TraceExporterConfig::Otlp { traces_endpoint } => {
  48             init_otel_or_log_fallback(service_name, &traces_endpoint)
  49         }
  50     }
```


---


```
  109     service_name: &'static str,
  110     traces_endpoint: &str,
  111 ) -> Result<ObservabilityGuard, Box<dyn std::error::Error + Send + Sync>> {
  112     let provider = build_tracer_provider(service_name, traces_endpoint)?;
! 113     let guard = install_tracer_subscriber(service_name, provider);
! 114     tracing::info!(
  115         service_name,
  116         otel_traces_endpoint = %traces_endpoint,
! 117         otel_protocol = %otel_protocol_label(),
  118         "OpenTelemetry OTLP tracing exporter enabled"
  119     );
! 120     Ok(guard)
! 121 }
  122 
  123 fn install_tracer_subscriber(
  124     service_name: &'static str,
  125     provider: SdkTracerProvider,
```


---


```
  158         .with_http()
  159         .with_endpoint(traces_endpoint)
  160         .with_protocol(otel_protocol())
  161         .build()?;
! 162     Ok(tracer_provider_builder(service_name)
! 163         .with_batch_exporter(exporter)
! 164         .build())
! 165 }
  166 
  167 fn tracer_provider_builder(service_name: &'static str) -> TracerProviderBuilder {
  168     let resource = Resource::builder()
  169         .with_service_name(service_name)
```


---


