# Diff Coverage
## Diff: 70078f649704a8a21cd025e3c5c2f992dd43f8d4...HEAD, staged, unstaged and untracked changes

- bike-rs/api/src/controllers/strava&#46;rs (89.7%): Missing lines 98-99,344-345

## Summary

- **Total**: 39 lines
- **Missing**: 4 lines
- **Coverage**: 89.74%



## bike-rs/api/src/controllers/strava&#46;rs

```
   94 pub async fn begin_connect(
   95     UserContext { user, .. }: UserContext<AppStorage>,
   96     State(state): State<Arc<AppStorage>>,
   97 ) -> Result<Response, AppError> {
!  98     authorize_response(&state.db, Config::get(), user.id).await
!  99 }
  100 
  101 async fn authorize_response(
  102     db: &DatabaseConnection,
  103     config: &Config,
```


---


```
  340 pub async fn handle_webhook_verification(
  341     State(state): State<Arc<AppStorage>>,
  342     Query(query): Query<strava::StravaWebhookSubscriptionQuery>,
  343 ) -> Result<Json<strava::StravaWebhookChallengeResponse>, AppError> {
! 344     verify_webhook_response(&state.db, Config::get(), query).await
! 345 }
  346 
  347 async fn verify_webhook_response(
  348     db: &DatabaseConnection,
  349     config: &Config,
```


---


