# Diff Coverage
## Diff: 6718a8f493d55e7c624283bedcba8a97808385c6...HEAD, staged, unstaged and untracked changes

- bike-ui/components/activity-detail/ActivityHeaderActions&#46;tsx (100%)
- bike-ui/components/admin/TaskProcessingHistory&#46;tsx (100%)
- bike-ui/components/imports/ImportDetail&#46;tsx (90.9%): Missing lines 33
- bike-ui/components/IntegrationEventFeed&#46;tsx (0.0%): Missing lines 128
- bike-ui/components/ui/TemporalLink&#46;tsx (100%)
- bike-ui/lib/config&#46;ts (100%)
- bike-ui/lib/queries&#46;ts (100%)

## Summary

- **Total**: 29 lines
- **Missing**: 2 lines
- **Coverage**: 93.1%



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


