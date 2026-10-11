# Diff Coverage
## Diff: fd2b7ea251144a1d3e2de9c1e24c332209da3550...HEAD, staged, unstaged and untracked changes

- strava-gateway/internal/storage/jobs&#46;go (100%)
- strava-gateway/internal/storage/sync&#46;go (60.0%): Missing lines 46-47

## Summary

- **Total**: 27 lines
- **Missing**: 2 lines
- **Coverage**: 92.59%



## strava-gateway/internal/storage/sync&#46;go

```
  42 // site link. The active-job constraint keeps concurrent workers from queuing
  43 // duplicate work, while the latest job timestamp bounds provider usage.
  44 func (syncs Syncs) QueueDueReconciliation(ctx context.Context, now time.Time) (int64, error) {
  45 	command, err := syncs.DB.Exec(ctx, `INSERT INTO gateway_sync_jobs
! 46 		(athlete_id,target,mode,after_epoch,reconciliation)
! 47 		SELECT links.athlete_id,links.target,'incremental',$2,true
  48 		FROM gateway_site_links AS links
  49 		JOIN gateway_connections AS connections ON connections.athlete_id=links.athlete_id
  50 		WHERE links.enabled AND links.target='rust' AND connections.revoked_at IS NULL
  51 		  AND NOT EXISTS (
```


---


