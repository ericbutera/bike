# Diff Coverage
## Diff: cd46e69f4c2c7148a0dd0a077a522ebe66bb82ae...HEAD, staged, unstaged and untracked changes

- bike-ui/app/admin/activities/page&#46;tsx (84.0%): Missing lines 107,204,210,221
- bike-ui/app/admin/integrations/page&#46;tsx (94.4%): Missing lines 32,107
- bike-ui/app/admin/tasks/page&#46;tsx (0.0%): Missing lines 23
- bike-ui/components/admin/ProcessorOverview&#46;tsx (68.2%): Missing lines 65-66,68-71,105
- bike-ui/components/admin/TasksPageContent&#46;tsx (76.2%): Missing lines 100,146,215,260-261,270,279,282,456,461
- bike-ui/components/imports/ImportHistory&#46;tsx (85.7%): Missing lines 63,65,67,197
- bike-ui/components/ui/DataGrid&#46;tsx (79.3%): Missing lines 30,33,40,52,60,100
- bike-ui/components/ui/GenericList&#46;tsx (75.0%): Missing lines 164

## Summary

- **Total**: 187 lines
- **Missing**: 35 lines
- **Coverage**: 81.28%



## bike-ui/app/admin/activities/page&#46;tsx

```
  103         className: "whitespace-nowrap",
  104         render: (activity) =>
  105           activity.import_status ? (
  106             <span
! 107               title={activity.import_processing_stage ?? undefined}
  108               className={importStatusClass(activity.import_status)}
  109             >
  110               {activity.import_status}
  111             </span>
```


---


```
  200               title="Activity owner ID"
  201               placeholder="Any user"
  202               className="input input-sm input-bordered w-32"
  203               value={params.user_id ?? ""}
! 204               onChange={(event) => setFilter("user_id", event.target.value)}
  205             />
  206             <select
  207               aria-label="Activity source"
  208               className="select select-sm select-bordered w-40"
```


---


```
  206             <select
  207               aria-label="Activity source"
  208               className="select select-sm select-bordered w-40"
  209               value={params.source ?? ""}
! 210               onChange={(event) => setFilter("source", event.target.value)}
  211             >
  212               <option value="">All sources</option>
  213               <option value="manual_upload">Upload</option>
  214               <option value="archive_url_import">Archive</option>
```


---


```
  217             <select
  218               aria-label="Activity sport"
  219               className="select select-sm select-bordered w-40"
  220               value={params.sport ?? ""}
! 221               onChange={(event) => setFilter("sport", event.target.value)}
  222             >
  223               <option value="">All sports</option>
  224               {ACTIVITY_SPORT_OPTIONS.map((sport) => (
  225                 <option key={sport.value} value={sport.value}>
```


---



## bike-ui/app/admin/integrations/page&#46;tsx

```
  28 function AdminIntegrationsContent() {
  29   const router = useRouter();
  30   const searchParams = useSearchParams();
  31   const [ids, setIds] = useState({
! 32     user_id: searchParams.get("user_id") ?? "",
  33     activity_id: searchParams.get("activity_id") ?? "",
  34     import_id: searchParams.get("import_id") ?? "",
  35   });
  36   const [provider, setProvider] = useState(
```


---


```
  103             <select
  104               aria-label="Provider"
  105               className="select select-sm select-bordered w-44"
  106               value={provider}
! 107               onChange={(event) => setProvider(event.target.value)}
  108             >
  109               <option value="">All providers</option>
  110               <option value="strava">Strava</option>
  111               <option value="activity_processing">Activity processing</option>
```


---



## bike-ui/app/admin/tasks/page&#46;tsx

```
  19   const params = useSearchParams();
  20   return (
  21     <div className="grid gap-6">
  22       <TasksPageContent
! 23         initialCorrelationId={params.get("correlation_id") ?? ""}
  24       />
  25     </div>
  26   );
  27 }
```


---



## bike-ui/components/admin/ProcessorOverview&#46;tsx

```
  61   );
  62   return (
  63     <ListCard
  64       title="All registered processors"
! 65       onSearch={() => {
! 66         void query.refetch();
  67       }}
! 68       onClear={() => {
! 69         setHours(24);
! 70         setOutcome("completed");
! 71         setSearch("");
  72       }}
  73       filters={
  74         <>
  75           <select
```


---


```
  101             aria-label="Search processors"
  102             placeholder="Search processors"
  103             className="input input-sm input-bordered w-56"
  104             value={search}
! 105             onChange={(event) => setSearch(event.target.value)}
  106           />
  107         </>
  108       }
  109     >
```


---



## bike-ui/components/admin/TasksPageContent&#46;tsx

```
   96     setFilters((current) => ({ ...current, [key]: value }));
   97     setPage(1);
   98   }
   99 
! 100   function applyFilters() {
  101     setFilters({ ...draft });
  102     setPage(1);
  103   }
```


---


```
  142       className: "whitespace-nowrap",
  143       render: (task) => (
  144         <span
  145           title={
! 146             task.durationMs === null
  147               ? undefined
  148               : Math.round(task.durationMs / 1000) + "s"
  149           }
  150         >
```


---


```
  211               aria-label="Filter error text"
  212               placeholder="Filter error text"
  213               className="input input-sm w-40"
  214               value={draft.error}
! 215               onChange={(event) => setDraftField("error", event.target.value)}
  216             />
  217             <select
  218               aria-label="Filter task type"
  219               className="select select-sm w-40"
```


---


```
  256               aria-label="From date"
  257               title="From"
  258               className="input input-sm w-40"
  259               value={draft.fromDate}
! 260               onChange={(event) =>
! 261                 setDraftField("fromDate", event.target.value)
  262               }
  263             />
  264             <input
  265               type="date"
```


---


```
  266               aria-label="To date"
  267               title="To"
  268               className="input input-sm w-40"
  269               value={draft.toDate}
! 270               onChange={(event) => setDraftField("toDate", event.target.value)}
  271             />
  272           </>
  273         }
  274       >
```


---


```
  275         <DataGrid
  276           rows={tasks}
  277           columns={columns}
  278           isLoading={query.isLoading}
! 279           error={query.error ? "Unable to load tasks." : undefined}
  280           emptyMessage="No tasks found matching criteria."
  281           label="Background task history"
! 282           onRowClick={(task) => setSelectedId(task.id)}
  283         />
  284         {total > 0 ? (
  285           <Pagination
  286             className="mt-4"
```


---


```
  452             >
  453               <FontAwesomeIcon icon={faRotateRight} /> Rerun
  454             </button>
  455           </li>
! 456           {canCancel ? (
  457             <li>
  458               <button
  459                 type="button"
  460                 disabled={pending}
! 461                 onClick={() => void onAction("cancel", task.id)}
  462               >
  463                 <FontAwesomeIcon icon={faBan} /> Cancel
  464               </button>
  465             </li>
```


---



## bike-ui/components/imports/ImportHistory&#46;tsx

```
  59     className: "max-w-xs truncate",
  60     render: (item) => (
  61       <span
  62         className={
! 63           item.processing_error ? "text-error" : "text-base-content/50"
  64         }
! 65         title={item.processing_error ?? undefined}
  66       >
! 67         {item.processing_error || "—"}
  68       </span>
  69     ),
  70   },
  71   {
```


---


```
  193           User ID must be positive
  194         </p>
  195       ) : (
  196         <DataGrid
! 197           rows={query.data?.items ?? []}
  198           columns={columns}
  199           isLoading={query.isLoading}
  200           error={query.error?.message}
  201           emptyMessage="No imports match these filters."
```


---



## bike-ui/components/ui/DataGrid&#46;tsx

```
  26 }) {
  27   return (
  28     <div className="card min-w-0 bg-base-100 shadow-xl">
  29       <div className="card-body min-w-0">
! 30         {title || actions || filters ? (
  31           <div className="mb-4 flex flex-col gap-4">
  32             <div className="flex flex-wrap items-center justify-between gap-3">
! 33               {title ? (
  34                 <h2 className="card-title m-0 text-2xl font-bold">{title}</h2>
  35               ) : null}
  36               {actions ? (
  37                 <div className="flex items-center gap-3">{actions}</div>
```


---


```
  36               {actions ? (
  37                 <div className="flex items-center gap-3">{actions}</div>
  38               ) : null}
  39             </div>
! 40             {filters ? (
  41               <form
  42                 className="flex w-full flex-wrap items-center justify-end gap-2 rounded-lg bg-base-200/50 p-2"
  43                 onSubmit={(event) => {
  44                   event.preventDefault();
```


---


```
  48                 <div className="flex flex-1 flex-wrap justify-end gap-2">
  49                   {filters}
  50                 </div>
  51                 <div className="join">
! 52                   {onSearch ? (
  53                     <button
  54                       className="join-item btn btn-sm btn-neutral"
  55                       type="submit"
  56                     >
```


---


```
  56                     >
  57                       Search
  58                     </button>
  59                   ) : null}
! 60                   {onClear ? (
  61                     <button
  62                       className="join-item btn btn-sm btn-ghost border-base-300"
  63                       type="button"
  64                       onClick={onClear}
```


---


```
   96   label?: string;
   97   onRowClick?: (row: T) => void;
   98 }) {
   99   if (isLoading)
! 100     return <div className="p-6 text-sm text-base-content/70">Loading...</div>;
  101   if (error)
  102     return (
  103       <div role="alert" className="p-6 text-sm text-error">
  104         {error}
```


---



## bike-ui/components/ui/GenericList&#46;tsx

```
  160         isLoading={result.isLoading}
  161         error={
  162           result.error instanceof Error
  163             ? result.error.message
! 164             : result.error
  165               ? "Unable to load this list."
  166               : undefined
  167         }
  168         emptyMessage={emptyMessage}
```


---


