# Diff Coverage
## Diff: cdb31e09a5c810f3b019a04508350ca85c8b7e62...HEAD, staged, unstaged and untracked changes

- bike-ui/components/reports/ReportsClient&#46;tsx (0.0%): Missing lines 2123
- bike-ui/components/segment-detail/SegmentDetailHeader&#46;tsx (50.0%): Missing lines 207
- bike-ui/lib/downloadFile&#46;ts (100%)
- bike-ui/lib/segmentGpx&#46;ts (100%)

## Summary

- **Total**: 38 lines
- **Missing**: 2 lines
- **Coverage**: 94.74%



## bike-ui/components/reports/ReportsClient&#46;tsx

```
  2119       headers.map((header) => csvCell(row[header] ?? "")).join(","),
  2120     ),
  2121   ].join("\n");
  2122   const blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
! 2123   downloadFile(filename, blob);
  2124 }
  2125 
  2126 function csvCell(value: string | number) {
  2127   const text = String(value);
```


---



## bike-ui/components/segment-detail/SegmentDetailHeader&#46;tsx

```
  203                 <li>
  204                   <button
  205                     type="button"
  206                     onClick={() => downloadSegmentGpx(segment)}
! 207                     disabled={(segment.route_points?.length ?? 0) < 2}
  208                   >
  209                     Download segment
  210                   </button>
  211                 </li>
```


---


