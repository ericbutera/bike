# Diff Coverage
## Diff: 9fb0145033ddf2ce425dd1aa948c931b0f092cf8...HEAD, staged, unstaged and untracked changes

- bike-rs/bike-core/src/archive_import&#46;rs (100%)
- bike-rs/bike-core/src/auth/controllers/admin&#46;rs (62.5%): Missing lines 156,162-163
- bike-rs/bike-core/src/auth/controllers/admin/audit&#46;rs (100%)
- bike-rs/bike-core/src/auth/controllers/oauth&#46;rs (0.0%): Missing lines 156
- bike-rs/bike-core/src/auth/entities/auth_events&#46;rs (100%)
- bike-rs/bike-core/src/auth/entities/users&#46;rs (100%)
- bike-rs/bike-core/src/auth/services&#46;rs (100%)
- bike-rs/bike-core/src/jobs/adapter&#46;rs (100%)
- bike-rs/bike-core/src/jobs/history&#46;rs (100%)
- bike-rs/bike-core/src/jobs/native&#46;rs (100%)
- bike-rs/bike-core/src/test_support/accounts&#46;rs (100%)

## Summary

- **Total**: 125 lines
- **Missing**: 4 lines
- **Coverage**: 96.8%



## bike-rs/bike-core/src/auth/controllers/admin&#46;rs

```
  152 pub async fn get_user<S>(
  153     _admin: AdminUserContext<S>,
  154     State(state): State<Arc<S>>,
  155     Path(id): Path<i32>,
! 156 ) -> Result<Json<AdminUserDetailsResponse>, AuthError>
  157 where
  158     S: AuthStorage + 'static,
  159 {
  160     let user = users::Model::find_by_id(state.db(), id).await?;
```


---


```
  158     S: AuthStorage + 'static,
  159 {
  160     let user = users::Model::find_by_id(state.db(), id).await?;
  161 
! 162     Ok(Json(AdminUserDetailsResponse::from(
! 163         user.admin_details(state.db()).await?,
  164     )))
  165 }
  166 
  167 #[utoipa::path(
```


---



## bike-rs/bike-core/src/auth/controllers/oauth&#46;rs

```
  152     S: OAuthRouteStorage,
  153 {
  154     let user = OAuthService::find_or_create_provider_user(state.db(), provider, provider_user).await?;
  155 
! 156     let tokens = state.session_service().login(state.db(), &user).await?;
  157     let frontend_url = state.frontend_url().to_string();
  158     let cookie_val = refresh_cookie_value(&tokens.refresh_token, &frontend_url);
  159 
  160     let redirect_url = format!("{}/auth/callback", frontend_url.trim_end_matches('/'));
```


---


