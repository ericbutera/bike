# Diff Coverage
## Diff: fc9bf1566dd44e6fe89206859d0c4b2d5164b63e...HEAD, staged, unstaged and untracked changes

- bike-ui/components/activity-detail/ActivityClassificationSelect&#46;tsx (84.2%): Missing lines 33,36,45
- bike-ui/components/activity-detail/ActivityClimbsPanel&#46;tsx (100%)
- bike-ui/components/activity-detail/ActivityDataCard&#46;tsx (100%)
- bike-ui/components/activity-detail/ActivityHeaderActions&#46;tsx (100%)
- bike-ui/components/activity-detail/ActivityLapsTable&#46;tsx (100%)
- bike-ui/components/activity-detail/ActivityModal&#46;tsx (98.2%): Missing lines 29
- bike-ui/components/activity-detail/ActivityRouteDetails&#46;tsx (97.4%): Missing lines 62
- bike-ui/components/activity-detail/ActivitySegmentLegend&#46;tsx (100%)
- bike-ui/components/activity-detail/ActivitySignalsCard&#46;tsx (78.6%): Missing lines 420,423,434
- bike-ui/components/activity-stream/ActivitySportFilter&#46;tsx (100%)
- bike-ui/components/activity-stream/ActivityStreamCard&#46;tsx (0.0%): Missing lines 62
- bike-ui/components/ActivityDetailPanel&#46;tsx (85.7%): Missing lines 177
- bike-ui/components/ActivitySportIcon&#46;tsx (94.1%): Missing lines 103
- bike-ui/components/icons/OffRoadBikeIcon&#46;tsx (100%)
- bike-ui/components/MatchedSegmentsSection&#46;tsx (66.7%): Missing lines 705-706,708-709,722-724,739-740,808
- bike-ui/components/ui/ActionMenu&#46;tsx (100%)

## Summary

- **Total**: 246 lines
- **Missing**: 21 lines
- **Coverage**: 91.46%



## bike-ui/components/activity-detail/ActivityClassificationSelect&#46;tsx

```
  29     ACTIVITY_SPORT_OPTIONS.find((option) => option.value === selected)?.label ??
  30     formatSport(value);
  31   return (
  32     <ActionMenu
! 33       label={`${iconOnly ? "Change activity type" : "Activity type"}: ${label}`}
  34       disabled={disabled}
  35       triggerClassName={
! 36         iconOnly
  37           ? "btn btn-ghost btn-square btn-sm"
  38           : "btn btn-sm justify-between border-base-300 bg-base-100 font-normal"
  39       }
  40       triggerContent={
```


---


```
  41         <>
  42           <span aria-hidden="true">
  43             <ActivitySportIcon sport={value} />
  44           </span>
! 45           {iconOnly ? null : (
  46             <>
  47               {label}
  48               <FontAwesomeIcon icon={faChevronDown} aria-hidden="true" />
  49             </>
```


---



## bike-ui/components/activity-detail/ActivityModal&#46;tsx

```
  25   onClose: () => void;
  26 }) {
  27   const sportLabelId = useId();
  28   const knownSport = parseActivitySport(
! 29     resolveActivitySport(activity.sport) ?? null,
  30   );
  31   const initial = {
  32     title: activity.title,
  33     sport: knownSport ?? activity.sport,
```


---



## bike-ui/components/activity-detail/ActivityRouteDetails&#46;tsx

```
  58       case "End":
  59         nextIndex = tabs.length - 1;
  60         break;
  61       default:
! 62         return;
  63     }
  64     event.preventDefault();
  65     setActiveTab(tabs[nextIndex].key);
  66     tabRefs.current[nextIndex]?.focus();
```


---



## bike-ui/components/activity-detail/ActivitySignalsCard&#46;tsx

```
  416                       type="linear"
  417                       dataKey={entry.key}
  418                       yAxisId={entry.key}
  419                       stroke={entry.strokeColor}
! 420                       fill={entry.fillColor ?? entry.strokeColor}
  421                       fillOpacity={0.08}
  422                       strokeOpacity={0.35}
! 423                       strokeWidth={entry.strokeWidth ?? 1.75}
  424                       dot={false}
  425                       connectNulls
  426                     />
  427                   ) : (
```


---


```
  430                       type="linear"
  431                       dataKey={entry.key}
  432                       yAxisId={entry.key}
  433                       stroke={entry.strokeColor}
! 434                       strokeWidth={entry.strokeWidth ?? 2}
  435                       dot={false}
  436                       connectNulls
  437                     />
  438                   ),
```


---



## bike-ui/components/activity-stream/ActivityStreamCard&#46;tsx

```
  58               </h3>
  59             </div>
  60 
  61             <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-sm text-base-content/60 sm:justify-end">
! 62               {activity.location ? <span>{activity.location}</span> : null}
  63               <time
  64                 className="whitespace-nowrap"
  65                 dateTime={activity.started_at}
  66               >
```


---



## bike-ui/components/ActivityDetailPanel&#46;tsx

```
  173         />
  174 
  175         {(activity.laps ?? []).length > 0 ? (
  176           <ActivityLapsTable
! 177             laps={activity.laps ?? []}
  178             unitSystem={unitSystem}
  179           />
  180         ) : (
  181           <div className="alert mt-5">
```


---



## bike-ui/components/ActivitySportIcon&#46;tsx

```
   99       {customBike ? (
  100         <OffRoadBikeIcon fatTires={normalizedSport === "fat_bike"} />
  101       ) : (
  102         <FontAwesomeIcon
! 103           icon={icon ?? faCircleQuestion}
  104           style={{ height: "100%", width: "100%" }}
  105           aria-hidden="true"
  106         />
  107       )}
```


---



## bike-ui/components/MatchedSegmentsSection&#46;tsx

```
  701                   tabIndex={0}
  702                   className="collapse-title grid grid-cols-[auto_minmax(0,1fr)_auto] items-start gap-3 px-3 py-3 pr-3 sm:px-4 sm:pr-4"
  703                   aria-expanded={isExpanded}
  704                   aria-controls={`${segmentGroup.anchorId}-details`}
! 705                   onClick={() => {
! 706                     toggleSegmentMatch(segmentGroup.segmentId);
  707                   }}
! 708                   onKeyDown={(event) => {
! 709                     handleToggleRow(segmentGroup.segmentId, event);
  710                   }}
  711                 >
  712                   <button
  713                     type="button"
```


---


```
  718                       isStarred ? "Unstar" : "Star"
  719                     } ${segmentGroup.segmentTitle}`}
  720                     aria-pressed={isStarred}
  721                     disabled={updateSegmentMutation.isPending}
! 722                     onClick={(event) => {
! 723                       event.stopPropagation();
! 724                       void toggleSegmentStar(
  725                         segmentGroup.segmentId,
  726                         !isStarred,
  727                       );
  728                     }}
```


---


```
  735                       <SegmentColorStatus color={segmentGroup.tone.mapColor} />
  736                       <Link
  737                         href={segmentHref}
  738                         className="truncate text-base font-semibold text-base-content transition hover:text-primary"
! 739                         onClick={(event) => {
! 740                           event.stopPropagation();
  741                         }}
  742                       >
  743                         {segmentGroup.segmentTitle}
  744                       </Link>
```


---


```
  804                         </span>
  805                         <span className="badge badge-outline">
  806                           Peak HR {formatHeartRate(segmentGroup.peakHeartRate)}
  807                         </span>
! 808                         {segmentAchievement ? (
  809                           <SegmentAchievementBadge
  810                             achievement={segmentAchievement}
  811                           />
  812                         ) : null}
```


---


