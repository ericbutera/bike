# Diff Coverage
## Diff: 7d811e85cc1f1992a99002b0d609e7cbd9d9bdef...HEAD, staged, unstaged and untracked changes

- bike-rs/api/src/controllers/activity_imports&#46;rs (100%)
- bike-rs/api/src/controllers/mod&#46;rs (100%)
- bike-rs/api/src/controllers/strava&#46;rs (4.3%): Missing lines 97-100,129-137,184-187,210,212-221,251,268-269,271-273,279,286,310-311,375,380-384

## Summary

- **Total**: 51 lines
- **Missing**: 44 lines
- **Coverage**: 13.73%



## bike-rs/api/src/controllers/strava&#46;rs

```
   93 )]
   94 pub async fn begin_connect(
   95     UserContext { user, .. }: UserContext<AppStorage>,
   96     State(state): State<Arc<AppStorage>>,
!  97 ) -> Result<Response, AppError> {
!  98     let authorization =
!  99         strava::create_authorization_url_for_user(&state.db, Config::get(), user.id).await;
! 100     let authorization = match authorization {
  101         Ok(url) => {
  102             record_strava_event_best_effort(
  103                 &state.db,
  104                 Some(user.id),
```


---


```
  125             return Err(error.into());
  126         }
  127     };
  128 
! 129     let cookie =
! 130         strava::oauth_state::session_cookie(&authorization.browser_session, &Config::get().api_url);
! 131     Ok((
! 132         [(header::SET_COOKIE, cookie)],
! 133         Json(StravaAuthorizeResponse {
! 134             authorization_url: authorization.url.to_string(),
! 135         }),
! 136     )
! 137         .into_response())
  138 }
  139 
  140 #[utoipa::path(
  141     get,
```


---


```
  180 pub async fn queue_sync(
  181     UserContext { user, .. }: UserContext<AppStorage>,
  182     State(state): State<Arc<AppStorage>>,
  183 ) -> Result<Json<StravaConnectionResponse>, AppError> {
! 184     bike_core::strava::events::request_sync(&state.db, user.id).await?;
! 185     let connection = strava::load_connection(&state.db, user.id)
! 186         .await?
! 187         .ok_or_else(|| AppError::not_found("Strava connection was removed"))?;
  188 
  189     Ok(Json(
  190         response_from_model(&state.db, Some(&connection)).await?,
  191     ))
```


---


```
  206 )]
  207 pub async fn queue_backfill(
  208     UserContext { user, .. }: UserContext<AppStorage>,
  209     State(state): State<Arc<AppStorage>>,
! 210     Json(input): Json<StravaBackfillRequest>,
  211 ) -> Result<Json<StravaConnectionResponse>, AppError> {
! 212     bike_core::strava::events::request_backfill(
! 213         &state.db,
! 214         user.id,
! 215         input.start_date,
! 216         input.end_date,
! 217     )
! 218     .await?;
! 219     let connection = strava::load_connection(&state.db, user.id)
! 220         .await?
! 221         .ok_or_else(|| AppError::not_found("Strava connection was removed"))?;
  222     Ok(Json(
  223         response_from_model(&state.db, Some(&connection)).await?,
  224     ))
  225 }
```


---


```
  247 pub async fn disconnect_connection(
  248     UserContext { user, .. }: UserContext<AppStorage>,
  249     State(state): State<Arc<AppStorage>>,
  250 ) -> Result<Json<auth_openapi::schemas::MessageResponse>, AppError> {
! 251     bike_core::strava::events::request_disconnect(&state.db, user.id).await?;
  252     Ok(Json(auth_openapi::schemas::MessageResponse {
  253         message: "Strava disconnect queued.".to_string(),
  254     }))
  255 }
```


---


```
  264 )]
  265 pub async fn handle_callback(
  266     State(state): State<Arc<AppStorage>>,
  267     Query(query): Query<StravaCallbackQuery>,
! 268     headers: HeaderMap,
! 269 ) -> Response {
  270     let result = async {
! 271         let browser = strava::oauth_state::browser_session(&headers, &Config::get().api_url)?;
! 272         strava::oauth_state::OAuthState::new(&Config::get().jwt_secret)
! 273             .callback(&state.db, &state.db, &query, browser, chrono::Utc::now())
  274             .await
  275             .map_err(AppError::from)
  276     }
  277     .await;
```


---


```
  275             .map_err(AppError::from)
  276     }
  277     .await;
  278 
! 279     let redirect = match result {
  280         Ok(connection) => Redirect::to(&strava::build_frontend_account_redirect(
  281             Config::get(),
  282             "connected",
  283             connection.last_sync_message.as_deref(),
```


---


```
  282             "connected",
  283             connection.last_sync_message.as_deref(),
  284         )),
  285         Err(error) => {
! 286             if query.is_incomplete() {
  287                 record_strava_event_best_effort(
  288                     &state.db,
  289                     None,
  290                     "oauth.connect_failed",
```


---


```
  306                 Some(&error.message),
  307             ))
  308         }
  309     };
! 310     let cookie = strava::oauth_state::clear_session_cookie(&Config::get().api_url);
! 311     ([(header::SET_COOKIE, cookie)], redirect).into_response()
  312 }
  313 
  314 #[utoipa::path(
  315     get,
```


---


```
  371     tag = "strava"
  372 )]
  373 pub async fn handle_webhook_event(
  374     State(state): State<Arc<AppStorage>>,
! 375     headers: axum::http::HeaderMap,
  376     body: axum::body::Bytes,
  377 ) -> Result<Json<auth_openapi::schemas::MessageResponse>, AppError> {
  378     let event: strava::StravaWebhookEvent = serde_json::from_slice(&body)
  379         .map_err(|_| AppError::bad_request("Invalid Strava webhook body"))?;
! 380     let request_id = headers
! 381         .get("x-request-id")
! 382         .and_then(|value| value.to_str().ok())
! 383         .map(str::to_owned);
! 384     bike_core::strava::events::accept_webhook(&state.db, &event, request_id).await?;
  385 
  386     Ok(Json(auth_openapi::schemas::MessageResponse {
  387         message: "ok".to_string(),
  388     }))
```


---


