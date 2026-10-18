# Diff Coverage
## Diff: 9fb0145033ddf2ce425dd1aa948c931b0f092cf8...HEAD, staged, unstaged and untracked changes

- bike-ui/app/admin/activities/page&#46;tsx (100%)
- bike-ui/components/admin/AdminTaskHistory&#46;tsx (100%)
- bike-ui/components/admin/AdminTaskMenu&#46;tsx (86.4%): Missing lines 62,68,115
- bike-ui/components/admin/AdminUserAuditDialog&#46;tsx (89.5%): Missing lines 27,59
- bike-ui/components/admin/AdminUserDetailsDialog&#46;tsx (53.8%): Missing lines 20,30,36,44,48,55
- bike-ui/components/admin/AdminUserEditDialog&#46;tsx (90.6%): Missing lines 32,49,79
- bike-ui/components/admin/AdminUsersPageContent&#46;tsx (80.0%): Missing lines 129,135
- bike-ui/components/admin/UserMaintenanceDialog&#46;tsx (87.5%): Missing lines 64-65,70,115
- bike-ui/components/imports/ImportHistory&#46;tsx (100%)
- bike-ui/components/IntegrationEventFeed&#46;tsx (68.8%): Missing lines 93,156,159,162,174
- bike-ui/components/ui/ActionMenu&#46;tsx (100%)
- bike-ui/components/ui/Pagination&#46;tsx (83.3%): Missing lines 24
- bike-ui/components/ui/ProcessingDetails&#46;tsx (100%)
- bike-ui/components/ui/RefreshButton&#46;tsx (100%)
- bike-ui/components/ui/StatusBadge&#46;tsx (100%)
- bike-ui/components/ui/StravaIcon&#46;tsx (100%)
- bike-ui/lib/adminQueryOptions&#46;ts (100%)
- bike-ui/lib/adminTaskHistory&#46;ts (100%)
- bike-ui/lib/queries&#46;ts (36.4%): Missing lines 1141-1142,2158-2159,2163,2165-2166

## Summary

- **Total**: 249 lines
- **Missing**: 33 lines
- **Coverage**: 86.75%



## bike-ui/components/admin/AdminTaskMenu&#46;tsx

```
  58 export function UserTaskMenu({ user }: { user: AdminUser }) {
  59   const [maintenance, setMaintenance] = useState<UserMaintenanceAction>();
  60   const [view, setView] = useState<"details" | "edit" | "audit">();
  61   return (
! 62     <div className="inline-flex" onClick={(event) => event.stopPropagation()}>
  63       <TaskMenu
  64         target={{
  65           kind: "user",
  66           id: user.id,
```


---


```
  64         target={{
  65           kind: "user",
  66           id: user.id,
  67           userId: user.id,
! 68           label: `${user.name || user.email} (#${user.id})`,
  69         }}
  70         options={userTaskOptions}
  71         extraItems={[
  72           {
```


---


```
  111       {maintenance ? (
  112         <UserMaintenanceDialog
  113           user={user}
  114           action={maintenance}
! 115           onClose={() => setMaintenance(undefined)}
  116         />
  117       ) : null}
  118     </div>
  119   );
```


---



## bike-ui/components/admin/AdminUserAuditDialog&#46;tsx

```
  23     header: "User agent",
  24     className: "max-w-xs truncate",
  25     render: (event) => (
  26       <span title={event.user_agent ?? undefined}>
! 27         {event.user_agent || "—"}
  28       </span>
  29     ),
  30   },
  31   {
```


---


```
  55   const query = useAdminUserAuditEvents(user.id, page);
  56   const metadata = query.data?.metadata;
  57   return (
  58     <Modal
! 59       title={`Audit events for ${user.name || user.email} (#${user.id})`}
  60       onClose={onClose}
  61       className="max-w-5xl"
  62     >
  63       <div className="mt-4">
```


---



## bike-ui/components/admin/AdminUserDetailsDialog&#46;tsx

```
  16   const query = useAdminUser(user.id);
  17   const detail = query.data;
  18   return (
  19     <Modal title={`User #${user.id} details`} onClose={onClose}>
! 20       {query.isLoading ? <p className="my-4">Loading user details...</p> : null}
  21       {query.error ? (
  22         <p role="alert" className="my-4 text-error">
  23           Unable to load user details.
  24         </p>
```


---


```
  26       {detail ? (
  27         <div className="my-4">
  28           <ProcessingFacts
  29             facts={[
! 30               { label: "Name", value: detail.name || "—" },
  31               { label: "Email", value: detail.email },
  32               {
  33                 label: "Status",
  34                 value: (
```


---


```
  32               {
  33                 label: "Status",
  34                 value: (
  35                   <StatusBadge
! 36                     status={detail.disabled ? "Disabled" : "Active"}
  37                   />
  38                 ),
  39               },
  40               {
```


---


```
  40               {
  41                 label: "Verified",
  42                 value: (
  43                   <StatusBadge
! 44                     status={detail.email_verified ? "Verified" : "Unverified"}
  45                   />
  46                 ),
  47               },
! 48               { label: "Admin user", value: detail.is_admin ? "Yes" : "No" },
  49               {
  50                 label: "Created",
  51                 value: formatActivityTimestamp(detail.created_at),
  52               },
```


---


```
  51                 value: formatActivityTimestamp(detail.created_at),
  52               },
  53               {
  54                 label: "Last login",
! 55                 value: detail.last_login_at
  56                   ? formatActivityTimestamp(detail.last_login_at)
  57                   : "—",
  58               },
  59             ]}
```


---



## bike-ui/components/admin/AdminUserEditDialog&#46;tsx

```
  28     event.preventDefault();
  29     setError(undefined);
  30     try {
  31       await updateUser.updateAsync(user.id, {
! 32         name: name || undefined,
  33         is_admin: isAdmin,
  34       });
  35       toast.success("User updated");
  36       onClose();
```


---


```
  45       await disableUser.disableAsync(user.id, true);
  46       setDisabled(true);
  47       toast.success("User account disabled");
  48     } catch {
! 49       setError("Failed to disable user account.");
  50     }
  51   }
  52 
  53   return (
```


---


```
  75           <input
  76             type="checkbox"
  77             className="checkbox checkbox-sm"
  78             checked={isAdmin}
! 79             onChange={(event) => setIsAdmin(event.target.checked)}
  80             disabled={busy}
  81           />
  82           <span>Admin user</span>
  83         </label>
```


---



## bike-ui/components/admin/AdminUsersPageContent&#46;tsx

```
  125             aria-label="Search users by name or email"
  126             placeholder="Search name or email"
  127             className="input input-sm input-bordered w-56"
  128             value={params.q ?? ""}
! 129             onChange={(event) => setFilter("q", event.target.value)}
  130           />
  131           <select
  132             aria-label="Filter users by account status"
  133             className="select select-sm select-bordered w-40"
```


---


```
  131           <select
  132             aria-label="Filter users by account status"
  133             className="select select-sm select-bordered w-40"
  134             value={params.disabled ?? ""}
! 135             onChange={(event) => setFilter("disabled", event.target.value)}
  136           >
  137             <option value="">All Statuses</option>
  138             <option value="false">Active</option>
  139             <option value="true">Disabled</option>
```


---



## bike-ui/components/admin/UserMaintenanceDialog&#46;tsx

```
  60         isArchive
  61           ? "Fetch provider export archives"
  62           : "Clean up duplicate activities"
  63       }
! 64       onClose={() => {
! 65         if (!pending) onClose();
  66       }}
  67     >
  68       <div className="mt-4 space-y-4">
  69         <p className="font-medium">
! 70           {user.name || user.email} (#{user.id})
  71         </p>
  72         <p className="text-sm text-base-content/75">
  73           {isArchive
  74             ? "Fetch a Garmin Connect or Strava export URL and import its supported activity files into this user's account."
```


---


```
  111               type="submit"
  112               className="btn btn-primary"
  113               disabled={pending}
  114             >
! 115               {pending
  116                 ? "Submitting…"
  117                 : isArchive
  118                   ? "Queue archive import"
  119                   : "Clean duplicates"}
```


---



## bike-ui/components/IntegrationEventFeed&#46;tsx

```
  89                   </span>
  90                 </td>
  91                 {showProvider ? (
  92                   <td className="align-top whitespace-nowrap">
! 93                     {event.provider === "strava" ? (
  94                       <StravaIcon />
  95                     ) : (
  96                       event.provider
  97                     )}
```


---


```
  152           <div className="space-y-3">
  153             <p className="text-sm leading-6 text-base-content/80">
  154               {event.message}
  155             </p>
! 156             {typeof payload?.temporal_workflow_id === "string" ? (
  157               <TemporalLink
  158                 workflowId={payload.temporal_workflow_id}
! 159                 namespace={String(payload.temporal_namespace ?? "bike")}
  160               />
  161             ) : null}
! 162             {payloadMetrics.length > 0 ? (
  163               <div className="flex flex-wrap gap-2">
  164                 {payloadMetrics.map((metric) => (
  165                   <span
  166                     key={`${event.id}-${metric.label}`}
```


---


```
  170                   </span>
  171                 ))}
  172               </div>
  173             ) : null}
! 174             {hasPayload ? (
  175               <details>
  176                 <summary className="cursor-pointer text-xs uppercase tracking-[0.18em] text-base-content/50">
  177                   Payload
  178                 </summary>
```


---



## bike-ui/components/ui/Pagination&#46;tsx

```
  20   visiblePages?: number;
  21 }) {
  22   const totalPages = cursor
  23     ? Math.max(cursor.knownPages, page + Number(cursor.hasNextPage))
! 24     : Math.max(1, Math.ceil((total ?? 0) / perPage));
  25   if (totalPages <= 1) return null;
  26 
  27   const handlePageClick = (nextPage: number) => {
  28     if (nextPage !== page && nextPage >= 1 && nextPage <= totalPages) {
```


---



## bike-ui/lib/queries&#46;ts

```
  1137     { ...ADMIN_QUERY_OPTIONS, enabled: id !== null },
  1138   );
  1139 }
  1140 
! 1141 export function useAdminUserAuditEvents(id: number, page: number) {
! 1142   return $typedApi.useQuery(
  1143     "get",
  1144     "/admin/users/{id}/audit-events",
  1145     { params: { path: { id }, query: { page, per_page: 20 } } },
  1146     ADMIN_QUERY_OPTIONS,
```


---


```
  2154     data: response.data as IntegrationEvent[] | undefined,
  2155   };
  2156 }
  2157 
! 2158 export function useAdminUserArchiveImport(userId: number) {
! 2159   const mutation = $typedApi.useMutation(
  2160     "post",
  2161     "/admin/users/{id}/archive-imports",
  2162   );
! 2163   return {
  2164     ...mutation,
! 2165     importAsync: (archiveUrl: string) =>
! 2166       mutation.mutateAsync({
  2167         params: { path: { id: userId } },
  2168         body: { archive_url: archiveUrl },
  2169       }),
  2170   };
```


---


