# Diff Coverage
## Diff: 2c75fafde14e1f7276074e745ebe8df2c5efd303...HEAD, staged, unstaged and untracked changes

- bike-ui/app/admin/activities/page&#46;tsx (100%)
- bike-ui/components/activity-detail/ActivityImportTracePanel&#46;tsx (64.3%): Missing lines 68,74,122,145,168,182,187,192,221-222
- bike-ui/components/admin/ActivityPipelines&#46;tsx (90.0%): Missing lines 20
- bike-ui/components/admin/PipelineEvidenceLinks&#46;tsx (100%)
- bike-ui/components/admin/PipelinePanel&#46;tsx (70.0%): Missing lines 28,109-112,198
- bike-ui/components/admin/PipelineTaskEvidence&#46;tsx (71.8%): Missing lines 59,74,96,113,118,140-141,146,150,154,158
- bike-ui/components/imports/ImportDetail&#46;tsx (100%)
- bike-ui/components/ui/ProcessingDetails&#46;tsx (100%)
- bike-ui/components/ui/ProcessingGraph&#46;tsx (100%)
- bike-ui/lib/taskTiming&#46;ts (100%)

## Summary

- **Total**: 174 lines
- **Missing**: 28 lines
- **Coverage**: 83.91%



## bike-ui/components/activity-detail/ActivityImportTracePanel&#46;tsx

```
  64                 Follow the connections from top to bottom. Select a stage for
  65                 its result and recovery action.
  66               </p>
  67             </div>
! 68             {!trace.attempts?.length ? (
  69               <p className="text-sm">
  70                 Historical import: stage summaries are unavailable until replay.
  71               </p>
  72             ) : null}
```


---


```
  70                 Historical import: stage summaries are unavailable until replay.
  71               </p>
  72             ) : null}
  73           </div>
! 74           {trace.attempts?.length ? (
  75             <section aria-label="Import attempts" className="space-y-3">
  76               <h4 className="font-semibold">Attempt history</h4>
  77               <div className="flex flex-wrap gap-2">
  78                 {trace.attempts.map((item, index) => (
```


---


```
  118                     },
  119                     {
  120                       label: "Source format and quality",
  121                       value:
! 122                         [
  123                           attempt.source.format?.toUpperCase(),
  124                           attempt.source.quality?.replaceAll("_", " "),
  125                         ]
  126                           .filter(Boolean)
```


---


```
  141                     {
  142                       label: "Finished",
  143                       value: formatTaskDate(attempt.finished_at) || null,
  144                     },
! 145                     ...(attempt.reused_attempt_id
  146                       ? [
  147                           {
  148                             label: "Earlier results",
  149                             value: `Reused from attempt #${attempt.reused_attempt_id}`,
```


---


```
  164               edges={trace.graph.edges}
  165               selected={selected?.id}
  166               onSelect={setSelectedStage}
  167             />
! 168             {selected ? (
  169               <section
  170                 aria-label="Selected stage"
  171                 className="space-y-4 rounded-box border border-base-300 p-4 xl:sticky xl:top-4"
  172               >
```


---


```
  178                   {selected.summary?.map((fact) => (
  179                     <li key={fact}>{fact}</li>
  180                   ))}
  181                 </ul>
! 182                 {!selected.summary?.length ? (
  183                   <p className="text-sm text-base-content/60">
  184                     No summary was recorded for this stage.
  185                   </p>
  186                 ) : null}
! 187                 {selected.error ? (
  188                   <p className="text-sm text-error" role="alert">
  189                     {selected.error}
  190                   </p>
  191                 ) : null}
! 192                 {selected.reused_attempt_id ? (
  193                   <p className="text-sm">
  194                     Reused from attempt #{selected.reused_attempt_id}
  195                   </p>
  196                 ) : null}
```


---


```
  217                     {
  218                       label: "Depends on",
  219                       value:
  220                         trace.graph.edges
! 221                           .filter((edge) => edge.to === selected.id)
! 222                           .map((edge) => stageLabel(nodes, edge.from))
  223                           .join(", ") || "No earlier stage",
  224                     },
  225                   ]}
  226                 />
```


---



## bike-ui/components/admin/ActivityPipelines&#46;tsx

```
  16     ? selected
  17     : query.data?.run_ids[0];
  18   return (
  19     <section aria-label="Activity processing pipelines" className="space-y-3">
! 20       {query.isLoading ? (
  21         <p className="flex items-center gap-2">
  22           <LoadingSpinner size="sm" />
  23           Loading worker runs…
  24         </p>
```


---



## bike-ui/components/admin/PipelinePanel&#46;tsx

```
  24   });
  25   const run = query.data;
  26   const task =
  27     run?.tasks.find((task) => task.id === taskId) ??
! 28     (selected == null ? run?.tasks[0] : undefined);
  29   const gateway =
  30     selected?.startsWith("gateway_") && run
  31       ? gatewaySteps(run)[Number(selected.split("_")[1])]
  32       : null;
```


---


```
  105               <button
  106                 type="button"
  107                 key={item.id}
  108                 aria-pressed={item.id === task?.id}
! 109                 className={`rounded-box border p-3 text-left ${item.id === task?.id ? "border-primary bg-primary/5" : "border-base-300"}`}
! 110                 onClick={() => {
! 111                   setSelected(`task_${item.id}`);
! 112                   setWorkAfter(0);
  113                 }}
  114               >
  115                 <span className="block text-sm font-semibold">
  116                   Task #{item.id}: {item.task_type.replaceAll("_", " ")}
```


---


```
  194                       { label: "Revision", value: output.revision },
  195                       {
  196                         label: "Published",
  197                         value:
! 198                           formatTaskDate(output.available_at) ||
  199                           "Pending or not recorded",
  200                       },
  201                     ]}
  202                   />
```


---



## bike-ui/components/admin/PipelineTaskEvidence&#46;tsx

```
  55             value: duration(run.received_at, task.attempts[0]?.started_at),
  56           },
  57           {
  58             label: "Parent task",
! 59             value: task.parent_task_id
  60               ? `#${task.parent_task_id}`
  61               : "No parent task",
  62           },
  63         ]}
```


---


```
  70           <ProcessingFacts
  71             facts={[
  72               {
  73                 label: "Step duration",
! 74                 value: duration(started, finished ?? (running ? now : null)),
  75               },
  76               {
  77                 label: "Receipt to step start",
  78                 value: duration(run.received_at, started),
```


---


```
   92               { label: "Work key", value: work.work_key },
   93               { label: "Revision", value: work.revision },
   94             ]}
   95           />
!  96           {work.error ? (
   97             <p role="alert" className="text-error">
   98               {work.error}
   99             </p>
  100           ) : null}
```


---


```
  109             {stage.summary.map((fact) => (
  110               <li key={fact}>{fact}</li>
  111             ))}
  112           </ul>
! 113           {stage.error ? (
  114             <p role="alert" className="text-error">
  115               {stage.error}
  116             </p>
  117           ) : null}
! 118           {stage.reused_attempt_id ? (
  119             <p className="text-sm">
  120               Reused from attempt #{stage.reused_attempt_id}
  121             </p>
  122           ) : null}
```


---


```
  136               {
  137                 label: "Duration",
  138                 value: duration(
  139                   attempt.started_at,
! 140                   attempt.finished_at ??
! 141                     (attempt.outcome === "running" ? now : null),
  142                 ),
  143               },
  144               {
  145                 label: "Started",
! 146                 value: formatTaskDate(attempt.started_at) || null,
  147               },
  148               {
  149                 label: "Heartbeat",
! 150                 value: formatTaskDate(attempt.heartbeat_at) || null,
  151               },
  152               {
  153                 label: "Progress",
! 154                 value: formatTaskDate(attempt.progress_at) || null,
  155               },
  156             ]}
  157           />
! 158           {attempt.error ? (
  159             <p className="mt-2 text-sm text-error" role="alert">
  160               {attempt.error}
  161             </p>
  162           ) : null}
```


---


