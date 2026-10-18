# Diff Coverage
## Diff: da8d8660b09968fa11146ee8dd6184aa83366896...HEAD, staged, unstaged and untracked changes

- bike-ui/components/activity-detail/ActivityClimbsCard&#46;tsx (80.0%): Missing lines 171
- bike-ui/components/activity-detail/ActivityRouteMap&#46;tsx (100%)
- bike-ui/components/activity-detail/ActivitySegmentLegend&#46;tsx (100%)
- bike-ui/components/activity-detail/SegmentColorStatus&#46;tsx (100%)
- bike-ui/components/ActivityDetailPanel&#46;tsx (100%)
- bike-ui/components/MapLibreRouteMapClient&#46;tsx (87.8%): Missing lines 932,1022-1024,1192
- bike-ui/components/MatchedSegmentsSection&#46;tsx (100%)
- bike-ui/components/routeMapClimbs&#46;ts (100%)
- bike-ui/components/RouteMapEndpoints&#46;ts (100%)
- bike-ui/components/RouteMapNavigationControl&#46;ts (100%)
- bike-ui/components/ui/Modal&#46;tsx (50.0%): Missing lines 29
- bike-ui/components/ViewRouteOnMap&#46;tsx (100%)
- bike-ui/lib/mapAppearance&#46;ts (100%)

## Summary

- **Total**: 241 lines
- **Missing**: 7 lines
- **Coverage**: 97.1%



## bike-ui/components/activity-detail/ActivityClimbsCard&#46;tsx

```
  167       return;
  168     }
  169 
  170     event.preventDefault();
! 171     setSelectedClimbId(climbId);
  172   }
  173 
  174   return (
  175     <AppCard id="activity-climbs-card">
```


---



## bike-ui/components/MapLibreRouteMapClient&#46;tsx

```
  928       type: "FeatureCollection",
  929       features:
  930         (routePoints?.length ?? 0) >= 2
  931           ? [
! 932               buildLineFeature("route", routePoints ?? [], {
  933                 color: routeColor,
  934               }),
  935             ]
  936           : [],
```


---


```
  1018       );
  1019     }
  1020 
  1021     if (interactive && showZoomControls) {
! 1022       const control = new RouteMapNavigationControl(overlayLayersRef.current);
! 1023       navigationControlRef.current = control;
! 1024       map.addControl(control, "top-right");
  1025     }
  1026 
  1027     const handleResize = () => {
  1028       if (resizeFrameRef.current != null) {
```


---


```
  1188           handlerForEvent(event)?.onClick?.();
  1189         };
  1190         const handleOverlayMouseEnter = (event: OverlayLayerEvent) => {
  1191           map.getCanvas().style.cursor =
! 1192             overlayHandlersRef.current.size > 0 ? "pointer" : "";
  1193           handlerForEvent(event)?.onMouseEnter?.();
  1194         };
  1195         const handleOverlayMouseMove = (event: OverlayLayerEvent) => {
  1196           handlerForEvent(event)?.onMouseEnter?.();
```


---



## bike-ui/components/ui/Modal&#46;tsx

```
  25       ref={dialogRef}
  26       className="modal"
  27       aria-labelledby={titleId}
  28       onClick={(event) => event.stopPropagation()}
! 29       onKeyDown={(event) => event.stopPropagation()}
  30       onCancel={(event) => {
  31         event.preventDefault();
  32         onClose();
  33       }}
```


---


