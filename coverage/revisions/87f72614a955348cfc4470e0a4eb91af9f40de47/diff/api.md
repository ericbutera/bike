# Diff Coverage
## Diff: 0c20debc99a0c5b6ef202e41ba83e03ae7127e65...HEAD, staged, unstaged and untracked changes

- bike-rs/api/src/app_error&#46;rs (100%)
- bike-rs/api/src/controllers/activity_maps&#46;rs (86.1%): Missing lines 49-62
- bike-rs/api/src/controllers/admin&#46;rs (100%)
- bike-rs/api/src/controllers/mod&#46;rs (100%)
- bike-rs/api/src/storage&#46;rs (97.8%): Missing lines 81

## Summary

- **Total**: 189 lines
- **Missing**: 15 lines
- **Coverage**: 92.06%



## bike-rs/api/src/controllers/activity_maps&#46;rs

```
  45     let service = state
  46         .map_images
  47         .as_ref()
  48         .ok_or_else(|| AppError::not_found("Activity maps are not configured"))?;
! 49     let image = service
! 50         .image(
! 51             &state.db,
! 52             user.id,
! 53             query.activity_id,
! 54             variant,
! 55             &style_version,
! 56             MapOptions {
! 57                 theme: query.theme,
! 58                 dpr: query.dpr,
! 59             },
! 60         )
! 61         .await?;
! 62     Ok(image_response(image, &headers))
  63 }
  64 
  65 fn image_response(image: bike_core::activity_maps::MapImage, headers: &HeaderMap) -> Response {
  66     let unchanged = headers
```


---



## bike-rs/api/src/storage&#46;rs

```
  77             None
  78         };
  79 
  80         Self {
! 81             map_images: create_map_image_service(Config::get()).await,
  82             heatmaps: std::sync::Arc::new(bike_core::heatmaps::service::HeatmapService::default()),
  83             db,
  84             tasks,
  85             feature_flags,
```


---


