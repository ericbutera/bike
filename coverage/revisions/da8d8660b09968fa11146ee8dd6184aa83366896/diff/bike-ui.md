# Diff Coverage
## Diff: 394f518cc12ab5dd623b454f093dad77c20d3398...HEAD, staged, unstaged and untracked changes

- bike-ui/app/admin/activities/page&#46;tsx (100%)
- bike-ui/app/admin/integrations/page&#46;tsx (100%)
- bike-ui/components/admin/AdminTaskDialog&#46;tsx (87.8%): Missing lines 38-39,57,65-66
- bike-ui/components/admin/AdminTaskHistory&#46;tsx (100%)
- bike-ui/components/admin/AdminTaskMenu&#46;tsx (92.9%): Missing lines 59,96
- bike-ui/components/admin/AdminUsersPageContent&#46;tsx (0.0%): Missing lines 92
- bike-ui/components/admin/taskActions&#46;ts (100%)
- bike-ui/components/ui/ActionMenu&#46;tsx (95.2%): Missing lines 72-73,76
- bike-ui/components/ui/TemporalLink&#46;tsx (100%)
- bike-ui/lib/adminTaskHistory&#46;ts (100%)
- bike-ui/lib/adminTasks&#46;ts (100%)
- bike-ui/lib/openapi/react-query/api&#46;ts (100%)

## Summary

- **Total**: 210 lines
- **Missing**: 11 lines
- **Coverage**: 94.76%



## bike-ui/components/admin/AdminTaskDialog&#46;tsx

```
  34     setError(null);
  35     let parameters: TaskParameters;
  36     if (option.task === "strava_backfill") {
  37       if (!after || !before || before < after) {
! 38         setError("Choose a start date and an end date on or after it.");
! 39         return;
  40       }
  41       const end = new Date(before + "T00:00:00Z");
  42       end.setUTCDate(end.getUTCDate() + 1);
  43       parameters = {
```


---


```
  53     try {
  54       setAccepted(await mutation.submit(parameters));
  55     } catch (failure) {
  56       setError(
! 57         handleApiError(failure).message ?? "Task submission failed. Try again.",
  58       );
  59     }
  60   }
```


---


```
  61 
  62   return (
  63     <Modal
  64       title={option.label}
! 65       onClose={() => {
! 66         if (!mutation.isPending) onClose();
  67       }}
  68     >
  69       <div className="mt-4 space-y-4">
  70         <p className="font-medium">{target.label}</p>
```


---



## bike-ui/components/admin/AdminTaskMenu&#46;tsx

```
  55       target={{
  56         kind: "user",
  57         id: user.id,
  58         userId: user.id,
! 59         label: `${user.name || user.email} (#${user.id})`,
  60       }}
  61       options={userTaskOptions}
  62     />
  63   );
```


---


```
   92       {selected ? (
   93         <AdminTaskDialog
   94           target={target}
   95           option={selected}
!  96           onClose={() => setSelected(null)}
   97         />
   98       ) : null}
   99     </div>
  100   );
```


---



## bike-ui/components/admin/AdminUsersPageContent&#46;tsx

```
  88       {
  89         key: "actions",
  90         header: "",
  91         className: "w-12 whitespace-nowrap",
! 92         render: (user) => <UserTaskMenu user={user} />,
  93       },
  94     ],
  95     [],
  96   );
```


---



## bike-ui/components/ui/ActionMenu&#46;tsx

```
  68       close();
  69       return;
  70     }
  71     if (event.key === "Tab") {
! 72       close();
! 73       return;
  74     }
  75     const buttons = Array.from(
! 76       menu.current?.querySelectorAll<HTMLButtonElement>(
  77         "button:not(:disabled)",
  78       ) ?? [],
  79     );
  80     const current = buttons.indexOf(
```


---


