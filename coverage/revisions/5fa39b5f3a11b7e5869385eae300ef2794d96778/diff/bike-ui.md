# Diff Coverage
## Diff: e401699acdc0b5280a64d13d689fc3efd11cdd43...HEAD, staged, unstaged and untracked changes

- bike-ui/components/activity-detail/ActivityHeaderActions&#46;tsx (100%)
- bike-ui/components/ActivityDetailPanel&#46;tsx (100%)
- bike-ui/components/imports/ImportDetail&#46;tsx (90.9%): Missing lines 33
- bike-ui/components/IntegrationEventFeed&#46;tsx (0.0%): Missing lines 128
- bike-ui/components/ui/TemporalLink&#46;tsx (100%)
- bike-ui/components/XcGoalsProgressPanel&#46;tsx (50.0%): Missing lines 545,547
- bike-ui/lib/config&#46;ts (100%)
- bike-ui/lib/queries&#46;ts (91.7%): Missing lines 1591

## Summary

- **Total**: 45 lines
- **Missing**: 5 lines
- **Coverage**: 88.89%



## bike-ui/components/imports/ImportDetail&#46;tsx

```
  29           Refresh
  30         </button>
  31       </div>
  32       {query.isLoading ? <LoadingCard /> : null}
! 33       {query.error ? (
  34         <ErrorCard
  35           error={query.error}
  36           fallback="Unable to load import details."
  37         />
```


---



## bike-ui/components/IntegrationEventFeed&#46;tsx

```
  124                     </p>
  125                     {typeof payload?.temporal_workflow_id === "string" ? (
  126                       <TemporalLink
  127                         workflowId={payload.temporal_workflow_id}
! 128                         namespace={String(payload.temporal_namespace ?? "bike")}
  129                       />
  130                     ) : null}
  131                     {payloadMetrics.length > 0 ? (
  132                       <div className="flex flex-wrap gap-2">
```


---



## bike-ui/components/XcGoalsProgressPanel&#46;tsx

```
  541   switch (status) {
  542     case "queued":
  543       return "Historical XC rides are queued for backfill now. Charts will populate as soon as the training analysis rebuild runs.";
  544     case "waiting":
! 545       return "Historical XC rides are queued for backfill.";
  546     case "running":
! 547       return "Historical XC backfill is rebuilding historical training metrics.";
  548     case "failed":
  549       return "Historical XC backfill failed. Save the goal again or queue the user-id XC backfill from admin analytics.";
  550     default:
  551       return null;
```


---



## bike-ui/lib/queries&#46;ts

```
  1587           queryKey: ["get", "/segments/{id}/comparison"],
  1588         }),
  1589       ]);
  1590 
! 1591       return result;
  1592     },
  1593   };
  1594 }
```


---


