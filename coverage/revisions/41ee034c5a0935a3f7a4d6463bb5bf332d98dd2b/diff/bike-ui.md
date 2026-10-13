# Diff Coverage
## Diff: 83498dc9713c86f8deba96555290751c6712047a...HEAD, staged, unstaged and untracked changes

- bike-ui/app/account/page&#46;tsx (100%)
- bike-ui/components/imports/ImportDetail&#46;tsx (100%)
- bike-ui/components/imports/ImportHistory&#46;tsx (100%)
- bike-ui/components/IntegrationEventFeed&#46;tsx (100%)
- bike-ui/components/StravaSyncHistory&#46;tsx (96.2%): Missing lines 66,95
- bike-ui/lib/queries&#46;ts (100%)

## Summary

- **Total**: 65 lines
- **Missing**: 2 lines
- **Coverage**: 96.92%



## bike-ui/components/StravaSyncHistory&#46;tsx

```
  62     return payload.stage === "complete"
  63       ? "Sync completed."
  64       : "Sync in progress.";
  65   }
! 66   return null;
  67 }
  68 
  69 function activityChange(event: IntegrationEvent): ActivityChange | null {
  70   if (event.event_type !== "workflow.stage") return null;
```


---


```
  91     activityId,
  92     stravaActivityId,
  93     activityName:
  94       typeof payload.activity_name === "string"
! 95         ? payload.activity_name.trim() || null
  96         : null,
  97     change,
  98   };
  99 }
```


---


