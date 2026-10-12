# Diff Coverage
## Diff: 801ff68437a6b8288c0d4dce0c17701bf092d6bc...HEAD, staged, unstaged and untracked changes

- bike-ui/app/account/page&#46;tsx (0.0%): Missing lines 439
- bike-ui/components/IntegrationEventFeed&#46;tsx (100%)
- bike-ui/components/StravaSyncHistory&#46;tsx (96.8%): Missing lines 45

## Summary

- **Total**: 39 lines
- **Missing**: 2 lines
- **Coverage**: 94.87%



## bike-ui/app/account/page&#46;tsx

```
  435                   </div>
  436                   <div className="mt-2 space-y-2 text-sm text-base-content/70">
  437                     <div>
  438                       Last sync:{" "}
! 439                       {stravaConnection.last_sync_finished_at
  440                         ? formatActivityTimestamp(
  441                             stravaConnection.last_sync_finished_at,
  442                           )
  443                         : "Not synced yet"}
```


---



## bike-ui/components/StravaSyncHistory&#46;tsx

```
  41         activityId,
  42         stravaActivityId,
  43         activityName:
  44           typeof payload.activity_name === "string"
! 45             ? payload.activity_name.trim() || null
  46             : null,
  47         change,
  48       },
  49     ];
```


---


