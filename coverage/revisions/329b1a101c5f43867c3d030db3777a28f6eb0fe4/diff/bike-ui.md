# Diff Coverage
## Diff: b1eb9bf77abdc2a16a239f818045c76ea2104e19...HEAD, staged, unstaged and untracked changes

- bike-ui/app/admin/imports/layout&#46;tsx (100%)
- bike-ui/components/activity-detail/ActivityHeaderActions&#46;tsx (100%)
- bike-ui/components/ActivityImportsPanel&#46;tsx (50.0%): Missing lines 304,307,420
- bike-ui/components/imports/ImportHistory&#46;tsx (100%)
- bike-ui/lib/queries&#46;ts (66.7%): Missing lines 1759

## Summary

- **Total**: 22 lines
- **Missing**: 4 lines
- **Coverage**: 81.82%



## bike-ui/components/ActivityImportsPanel&#46;tsx

```
  300                             {formatActivityImportStatus(activityImport)}
  301                           </span>
  302                         </td>
  303                         <td className="min-w-[12rem] max-w-xs">
! 304                           {isAdmin ? (
  305                             <Link
  306                               href={`/admin/imports/${activityImport.id}`}
! 307                               onClick={(event) => event.stopPropagation()}
  308                               className="link truncate font-medium text-base-content"
  309                             >
  310                               {activityImport.original_filename}
  311                             </Link>
```


---


```
  416                       <div className="min-w-0 flex-1">
  417                         <div className="text-sm text-base-content/60">
  418                           Queued {formatActivityTimestamp(job.created_at)}
  419                         </div>
! 420                         {isAdmin ? (
  421                           <Link
  422                             className="link link-primary"
  423                             href={`/admin/imports?archive_job_id=${job.id}`}
  424                           >
```


---



## bike-ui/lib/queries&#46;ts

```
  1755   const enabled =
  1756     (opts?.enabled ?? true) &&
  1757     Number.isFinite(numericImportId) &&
  1758     numericImportId > 0;
! 1759   const path = "/admin/activity-imports/{id}/trace";
  1760   const response = $api.useQuery(
  1761     "get",
  1762     path,
  1763     {
```


---


