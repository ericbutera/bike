# Diff Coverage
## Diff: c389d83d3ec3222fc20768d61514f34f71eabeb9...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/activity_maps/service&#46;rs (87.5%): Missing lines 75
- bike-rs/bike-core/src/analytics&#46;rs (100%)
- bike-rs/bike-core/src/heatmaps/preparation&#46;rs (0.0%): Missing lines 85-86,88-89
- bike-rs/bike-core/src/heatmaps/service&#46;rs (80.0%): Missing lines 195,293
- bike-rs/bike-core/src/observability&#46;rs (92.1%): Missing lines 169-171
- bike-rs/bike-core/src/observability/database_spans&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/fixture&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/workflow_spans&#46;rs (100%)
- bike-rs/bike-core/src/strava/native/strava_native_unit/workflow_spans/sync&#46;rs (100%)
- bike-rs/bike-core/src/test_support/trace&#46;rs (100%)

## Summary

- **Total**: 440 lines
- **Missing**: 10 lines
- **Coverage**: 97.73%



## bike-rs/bike-core/src/activity_maps/service&#46;rs

```
  71                 bike.operation = "map.render.wait"
  72             ))
  73             .await;
  74         if let Some(png) = self.cache.read(key.clone()).await? {
! 75             tracing::Span::current().record("cache.hit", true);
  76             return Ok(cached(png));
  77         }
  78         tracing::Span::current().record("cache.hit", false);
  79         METRICS.misses.inc();
```


---



## bike-rs/bike-core/src/heatmaps/preparation&#46;rs

```
  81             ..Default::default()
  82         });
  83     }
  84     let points = derived.route_points.len() as u64;
! 85     let chunks = tokio::task::spawn_blocking(crate::observability::blocking_with_current_trace(
! 86         move || prepare_route(&derived),
  87     ))
! 88     .await
! 89     .map_err(|error| DbErr::Custom(error.to_string()))?;
  90     let published = Projection::publish(db, &pending, &chunks).await?;
  91     Ok(WorkCounts {
  92         inputs_read: points,
  93         units_computed: points,
```


---



## bike-rs/bike-core/src/heatmaps/service&#46;rs

```
  191             }
  192         };
  193         let _guard = lock.lock().await;
  194         if let Some(image) = self.cached(&key).await {
! 195             tracing::Span::current().record("cache.hit", true);
  196             return Ok(image);
  197         }
  198         tracing::Span::current().record("cache.hit", false);
  199         let permit = self
```


---


```
  289         if page.is_empty() {
  290             break;
  291         }
  292         for chunk in page {
! 293             chunks += 1;
  294             if Instant::now() >= deadline {
  295                 return Err(HeatmapError::Busy);
  296             }
  297             if activity != Some(chunk.activity_id) {
```


---



## bike-rs/bike-core/src/observability&#46;rs

```
  165         .with_endpoint(traces_endpoint)
  166         .with_protocol(otel_protocol())
  167         .build()?;
  168     Ok(tracer_provider_builder(service_name)
! 169         .with_span_processor(database_spans::DatabaseSpanProcessor::new(
! 170             BatchSpanProcessor::builder(exporter).build(),
! 171         ))
  172         .build())
  173 }
  174 
  175 fn tracer_provider_builder(service_name: &'static str) -> TracerProviderBuilder {
```


---


