# Diff Coverage
## Diff: b1eb9bf77abdc2a16a239f818045c76ea2104e19...HEAD, staged, unstaged and untracked changes

- bike-rs/api/src/activity_import_history&#46;rs (100%)
- bike-rs/api/src/controllers/activity_import_history&#46;rs (100%)
- bike-rs/api/src/controllers/admin&#46;rs (75.0%): Missing lines 262-263

## Summary

- **Total**: 43 lines
- **Missing**: 2 lines
- **Coverage**: 95.35%



## bike-rs/api/src/controllers/admin&#46;rs

```
  258     _admin: AdminUserContext<AppStorage>,
  259     State(state): State<Arc<AppStorage>>,
  260     Path(import_id): Path<i32>,
  261 ) -> Result<Json<activity_imports_controller::ActivityImportTraceResponse>, AppError> {
! 262     let import =
! 263         crate::activity_import_history::admin_import_for_inspection(&state.db, import_id).await?;
  264     let trace = build_admin_activity_import_trace(&state.db, import, None).await?;
  265 
  266     Ok(Json(trace))
  267 }
```


---


