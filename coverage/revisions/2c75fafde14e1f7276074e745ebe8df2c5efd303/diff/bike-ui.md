# Diff Coverage
## Diff: 55d578673c7abc9f859768b72205856ed1dcea95...HEAD, staged, unstaged and untracked changes

- bike-ui/app/activity-previews/route&#46;ts (100%)
- bike-ui/components/HeatmapMap&#46;tsx (100%)
- bike-ui/components/MapLibreRouteMapClient&#46;tsx (78.9%): Missing lines 620,622,911,946
- bike-ui/components/segment-detail/SegmentRaceViewer&#46;tsx (100%)
- bike-ui/lib/mapAppearance&#46;ts (100%)
- bike-ui/lib/routePreview&#46;ts (100%)

## Summary

- **Total**: 30 lines
- **Missing**: 4 lines
- **Coverage**: 86.67%



## bike-ui/components/MapLibreRouteMapClient&#46;tsx

```
  616           "line-cap": "round",
  617           "line-join": "round",
  618         },
  619         paint: {
! 620           "line-color": sharedBasemap ? "#ffffff" : "#10212d",
  621           "line-width": 9,
! 622           "line-opacity": sharedBasemap ? 1 : 0.86,
  623         },
  624       },
  625       beforeLabels,
  626     );
```


---


```
  907       return buildBasemapStyle(selectedBasemap, theme);
  908     }
  909 
  910     if (canShowLayerPicker) {
! 911       return buildBasemapStyle(selectedBasemap, theme);
  912     }
  913 
  914     if (customMapStyle && defaultBasemap !== DEFAULT_ROUTE_BASEMAP) {
  915       return customMapStyle;
```


---


```
  942     if (canShowLayerPicker) {
  943       return `basemap:${selectedBasemap}`;
  944     }
  945 
! 946     if (customMapStyle && defaultBasemap !== DEFAULT_ROUTE_BASEMAP) {
  947       return `custom:${customMapStyle}`;
  948     }
  949 
  950     return `basemap:${configuredBasemap}`;
```


---


