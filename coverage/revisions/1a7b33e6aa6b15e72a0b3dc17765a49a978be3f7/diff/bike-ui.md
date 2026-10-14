# Diff Coverage
## Diff: 7d811e85cc1f1992a99002b0d609e7cbd9d9bdef...HEAD, staged, unstaged and untracked changes

- bike-ui/components/IntegrationEventFeed&#46;tsx (75.0%): Missing lines 129
- bike-ui/components/StravaEventHistory&#46;tsx (100%)

## Summary

- **Total**: 8 lines
- **Missing**: 1 line
- **Coverage**: 87.5%



## bike-ui/components/IntegrationEventFeed&#46;tsx

```
  125                       <a
  126                         className="link link-primary"
  127                         target="_blank"
  128                         rel="noreferrer"
! 129                         href={`${process.env.NEXT_PUBLIC_TEMPORAL_UI_URL ?? "http://localhost:8080"}/namespaces/${encodeURIComponent(String(payload.temporal_namespace ?? "bike"))}/workflows/${encodeURIComponent(payload.temporal_workflow_id)}`}
  130                       >
  131                         View job in Temporal
  132                       </a>
  133                     ) : null}
```


---


