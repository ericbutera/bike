use crate::config::Config;
use crate::observability;
use crate::provider_metrics as metrics;
use crate::provider_rate_limit::{
    reconcile_provider_quota_usage, reserve_provider_quota, ProviderQuotaBucketSpec,
    ProviderQuotaReservation,
};
use crate::strava_objects::{ObjectStore, ResponseObject};
use crate::strava_provider_payload::{StravaActivityStreams, StravaActivitySummary};
use crate::workflow_error::WorkflowError as AppError;
use axum::http::StatusCode;
use chrono::{DateTime, Duration, Utc};
use reqwest::header::{HeaderMap, RETRY_AFTER};
use reqwest::Client;
use sea_orm::DatabaseConnection;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use strava_sdk::apis::{activities_api, configuration::Configuration, strava_api, streams_api};
use tracing::field;
use tracing::Instrument;

const STRAVA_PROVIDER: &str = "strava";
const STRAVA_OVERALL_15_MINUTE_BUCKET: &str = "overall_15_minute";
const STRAVA_OVERALL_DAILY_BUCKET: &str = "overall_daily";
const STRAVA_READ_15_MINUTE_BUCKET: &str = "read_15_minute";
const STRAVA_READ_DAILY_BUCKET: &str = "read_daily";
const STRAVA_OVERALL_15_MINUTE_LIMIT: i32 = 400;
const STRAVA_OVERALL_DAILY_LIMIT: i32 = 4_000;
const STRAVA_READ_15_MINUTE_LIMIT: i32 = 200;
const STRAVA_READ_DAILY_LIMIT: i32 = 2_000;

#[derive(Clone)]
pub(crate) struct StravaApiClient {
    client: reqwest_middleware::ClientWithMiddleware,
    db: DatabaseConnection,
    config: &'static Config,
    base_url: String,
}

#[derive(Clone)]
pub(crate) struct RequestArchive {
    pub store: ObjectStore,
    pub key: String,
}
tokio::task_local! { static ARCHIVE: RequestArchive; }
impl RequestArchive {
    pub(crate) async fn scope<F: std::future::Future>(self, future: F) -> F::Output {
        ARCHIVE.scope(self, future).await
    }
    fn current() -> Option<Self> {
        ARCHIVE.try_with(Clone::clone).ok()
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct StravaAuthorizationTokenResponse {
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    pub(crate) expires_at: i64,
    pub(crate) scope: String,
    pub(crate) athlete: StravaAthleteSummary,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StravaRefreshTokenResponse {
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    pub(crate) expires_at: i64,
    pub(crate) scope: Option<String>,
    pub(crate) athlete: Option<StravaAthleteSummary>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StravaAthleteSummary {
    pub(crate) id: i64,
    pub(crate) username: Option<String>,
    pub(crate) firstname: Option<String>,
    pub(crate) lastname: Option<String>,
    pub(crate) profile_medium: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StravaFault {
    message: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum StravaRequestClass {
    OverallOnly,
    Read,
}

impl StravaRequestClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::OverallOnly => "overall_only",
            Self::Read => "read",
        }
    }

    fn bucket_list(self) -> &'static str {
        match self {
            Self::OverallOnly => "overall_15_minute,overall_daily",
            Self::Read => "overall_15_minute,overall_daily,read_15_minute,read_daily",
        }
    }
}

impl StravaApiClient {
    pub(crate) fn new(db: &DatabaseConnection, config: &'static Config) -> Result<Self, AppError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .user_agent(format!("{}/strava-sync", config.app_name))
            .build()
            .map_err(|error| {
                tracing::error!(error = ?error, "failed to build Strava HTTP client");
                AppError::internal("Failed to initialize Strava integration")
            })?;

        Ok(Self {
            client: reqwest_middleware::ClientBuilder::new(client)
                .with(crate::strava_transport::ExchangeRecorder {
                    store: ObjectStore::new(&config.uploads_dir),
                })
                .build(),
            db: db.clone(),
            config,
            base_url: if config.environment == "production" {
                "https://www.strava.com".into()
            } else {
                std::env::var("STRAVA_API_ORIGIN")
                    .unwrap_or_else(|_| "https://www.strava.com".into())
            },
        })
    }

    fn sdk(&self, access_token: Option<&str>) -> Configuration {
        Configuration {
            client: self.client.clone(),
            base_path: format!("{}/api/v3", self.base_url),
            oauth_access_token: access_token.map(str::to_owned),
            user_agent: Some(format!("{}/strava-sync", self.config.app_name)),
            ..Configuration::default()
        }
    }

    fn oauth_sdk(&self, access_token: Option<&str>) -> Configuration {
        Configuration {
            base_path: self.base_url.clone(),
            ..self.sdk(access_token)
        }
    }

    pub(crate) async fn exchange_authorization_code(
        &self,
        code: &str,
    ) -> Result<StravaAuthorizationTokenResponse, AppError> {
        let archive = RequestArchive {
            store: ObjectStore::new(&self.config.uploads_dir),
            key: format!(
                "strava/credentials/authorization/{}",
                crate::strava_objects::checksum(code.as_bytes())
            ),
        };
        archive
            .scope(self.send_json(
                strava_api::oauth_token(
                    &self.oauth_sdk(None),
                    &self.config.strava_client_id,
                    &self.config.strava_client_secret,
                    "authorization_code",
                    Some(code),
                    None,
                ),
                StravaRequestClass::OverallOnly,
                "exchange_authorization_code",
                "exchange a Strava authorization code",
            ))
            .await
    }

    pub(crate) async fn refresh_access_token(
        &self,
        refresh_token: &str,
    ) -> Result<StravaRefreshTokenResponse, AppError> {
        self.send_json(
            strava_api::oauth_token(
                &self.oauth_sdk(None),
                &self.config.strava_client_id,
                &self.config.strava_client_secret,
                "refresh_token",
                None,
                Some(refresh_token),
            ),
            StravaRequestClass::OverallOnly,
            "refresh_access_token",
            "refresh a Strava access token",
        )
        .await
    }

    pub(crate) async fn get_activity_streams(
        &self,
        access_token: &str,
        activity_id: i64,
    ) -> Result<StravaActivityStreams, AppError> {
        self.send_json(
            streams_api::get_activity_streams(&self.sdk(Some(access_token)), activity_id, "time,distance,latlng,altitude,velocity_smooth,heartrate,cadence,watts,temp,moving,grade_smooth".split(',').map(str::to_owned).collect(), true),
            StravaRequestClass::Read,
            "get_activity_streams",
            "fetch Strava activity streams",
        )
        .await
    }

    pub(crate) async fn deauthorize(&self, access_token: &str) -> Result<(), AppError> {
        self.send_json::<serde_json::Value, _, _>(
            strava_api::deauthorize(&self.oauth_sdk(Some(access_token))),
            StravaRequestClass::OverallOnly,
            "deauthorize",
            "deauthorize the Strava app",
        )
        .await
        .map(|_| ())
    }

    async fn send_json<T, R, E>(
        &self,
        request: impl std::future::Future<Output = Result<R, E>>,
        request_class: StravaRequestClass,
        operation: &'static str,
        action: &str,
    ) -> Result<T, AppError>
    where
        T: DeserializeOwned,
        R: DeserializeOwned,
    {
        if let Some(archive) = RequestArchive::current() {
            if let Some(saved) = archive.store.response(&archive.key).await? {
                tracing::info!(operation, object_path = %saved.body.path, outcome = "reused", "Reusing saved Strava response");
                let bytes = archive.store.response_bytes(&saved).await?;
                // Replays honor the generated response model as live calls do.
                let _: R = decode_response(&bytes, action)?;
                return decode_response(&bytes, action);
            }
        }
        if !crate::strava_workflow::provider_work_enabled() {
            return Err(AppError::too_many_requests(
                "Strava provider work is paused for cutover",
                Some(Utc::now() + Duration::minutes(1)),
            ));
        }
        let span = tracing::info_span!(
            "strava.client.call",
            "otel.kind" = "client",
            provider = STRAVA_PROVIDER,
            operation,
            request_class = request_class.as_str(),
            bucket = request_class.bucket_list(),
            status = field::Empty,
            retry_at = field::Empty,
            trace_id = field::Empty,
            span_id = field::Empty,
        );

        async move {
            observability::record_current_trace_context();
            self.reserve_quota(request_class, operation).await?;
            let http_span = tracing::info_span!(
                "strava.http.request",
                "otel.kind" = "client",
                provider = STRAVA_PROVIDER,
                operation,
                request_class = request_class.as_str(),
                bucket = request_class.bucket_list(),
                status = field::Empty,
                retry_at = field::Empty,
                trace_id = field::Empty,
                span_id = field::Empty,
            );
            let response = async move {
                observability::record_current_trace_context();
                crate::strava_transport::capture(request).await
            }
            .instrument(http_span.clone())
            .await;
            observability::record_span_trace_context(&http_span);
            match &response.1 {
                Some(response) => {
                    let status_label = response.status.as_u16().to_string();
                    http_span.record("status", field::display(&status_label));
                    if let Some(retry_at) = parse_retry_after_header(&response.headers, Utc::now())
                    {
                        http_span.record("retry_at", field::display(retry_at));
                    }
                }
                None => {
                    http_span.record("status", "transport_error");
                }
            }
            self.parse_json_response(
                response.1,
                response.0.is_ok(),
                request_class,
                operation,
                action,
            )
            .await
        }
        .instrument(span)
        .await
    }

    async fn reserve_quota(
        &self,
        request_class: StravaRequestClass,
        operation: &'static str,
    ) -> Result<(), AppError> {
        let span = tracing::info_span!(
            "provider.quota.reserve",
            provider = STRAVA_PROVIDER,
            operation,
            request_class = request_class.as_str(),
            bucket = field::Empty,
            retry_at = field::Empty,
            decision = field::Empty,
            trace_id = field::Empty,
            span_id = field::Empty,
        );

        async move {
            observability::record_current_trace_context();
            let specs = strava_quota_specs(request_class);
            match reserve_provider_quota(&self.db, &specs).await? {
                ProviderQuotaReservation::Reserved => {
                    tracing::Span::current().record("decision", "reserved");
                    Ok(())
                }
                ProviderQuotaReservation::RateLimited(pause) => {
                    let current = tracing::Span::current();
                    current.record("decision", "rate_limited");
                    current.record("bucket", field::display(&pause.bucket));
                    current.record("retry_at", field::display(pause.retry_at));
                    metrics::record_provider_rate_limit_pause(
                        STRAVA_PROVIDER,
                        &pause.bucket,
                        operation,
                    );
                    tracing::warn!(
                        provider = %pause.provider,
                        bucket = %pause.bucket,
                        retry_at = %pause.retry_at,
                        operation,
                        "Strava request paused by local provider rate limiter"
                    );
                    Err(AppError::too_many_requests(
                        format!(
                            "Strava rate limit bucket {} is exhausted. Retry after {}.",
                            pause.bucket, pause.retry_at
                        ),
                        Some(pause.retry_at),
                    ))
                }
            }
        }
        .instrument(span)
        .await
    }

    async fn parse_json_response<T>(
        &self,
        response: Option<crate::strava_transport::RawResponse>,
        sdk_accepted: bool,
        request_class: StravaRequestClass,
        operation: &'static str,
        action: &str,
    ) -> Result<T, AppError>
    where
        T: DeserializeOwned,
    {
        let response = response.ok_or_else(|| {
            tracing::Span::current().record("status", "transport_error");
            metrics::record_provider_api_request(
                STRAVA_PROVIDER,
                operation,
                request_class.as_str(),
                "transport_error",
            );
            tracing::error!(
                action,
                operation,
                "Generated Strava SDK request failed; inspect the retained exchange"
            );
            AppError::internal(format!("Failed to {action}"))
        })?;
        let status = response.status;
        let status_label = status.as_u16().to_string();
        tracing::Span::current().record("status", field::display(&status_label));
        metrics::record_provider_api_request(
            STRAVA_PROVIDER,
            operation,
            request_class.as_str(),
            &status_label,
        );
        reconcile_strava_rate_limit_headers(&self.db, &response.headers, request_class).await;
        let retry_after = parse_retry_after_header(&response.headers, Utc::now());

        let headers = response.headers;
        let bytes = response.bytes;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(strava_error_from_response(
                status,
                &headers,
                &body,
                retry_after,
                request_class,
                operation,
                Utc::now(),
            ));
        }

        if !sdk_accepted {
            return Err(AppError::internal(format!("Generated Strava SDK rejected the response while trying to {action}; inspect the retained exchange")));
        }
        decode_response(&bytes, action)
    }

    pub(crate) async fn get_activity(
        &self,
        access_token: &str,
        activity_id: i64,
    ) -> Result<StravaActivitySummary, AppError> {
        self.send_json(
            activities_api::get_activity_by_id(
                &self.sdk(Some(access_token)),
                activity_id,
                Some(false),
            ),
            StravaRequestClass::Read,
            "get_activity",
            "fetch a Strava activity",
        )
        .await
    }

    pub(crate) async fn list_activity_page(
        &self,
        access_token: &str,
        after: Option<i64>,
        page: i32,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        let after = after
            .map(i32::try_from)
            .transpose()
            .map_err(|_| AppError::bad_request("Strava listing timestamp is out of range"))?;
        self.send_json(
            activities_api::get_logged_in_athlete_activities(
                &self.sdk(Some(access_token)),
                None,
                after,
                Some(page),
                Some(100),
            ),
            StravaRequestClass::Read,
            "list_activities",
            "list Strava activities",
        )
        .await
    }
}

fn decode_response<T: DeserializeOwned>(bytes: &[u8], action: &str) -> Result<T, AppError> {
    serde_json::from_slice(bytes).map_err(|error| {
        AppError::internal(format!(
            "Failed to parse saved Strava response while trying to {action}: {error}"
        ))
    })
}

pub(crate) async fn retain_response(
    store: &ObjectStore,
    fallback_prefix: &str,
    response: &crate::strava_transport::RawResponse,
    encrypted: bool,
) -> Result<Option<ResponseObject>, AppError> {
    let archive = RequestArchive::current().unwrap_or_else(|| RequestArchive {
        store: store.clone(),
        key: format!("{fallback_prefix}/response"),
    });
    let status = response.status;
    let key = if status.is_success() {
        archive.key
    } else {
        format!("{}/attempts/{}", archive.key, uuid::Uuid::new_v4())
    };
    let retained_headers = response
        .headers
        .iter()
        .filter(|(name, _)| {
            name.as_str().starts_with("x-")
                || ["content-type", "retry-after"].contains(&name.as_str())
        })
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.to_string(), value.to_owned()))
        })
        .collect();
    let saved = if encrypted {
        archive
            .store
            .save_encrypted_response(&key, status.as_u16(), retained_headers, &response.bytes)
            .await?
    } else {
        archive
            .store
            .save_response(&key, status.as_u16(), retained_headers, &response.bytes)
            .await?
    };
    tracing::info!(object_path = %saved.body.path, status = status.as_u16(), outcome = "fetched", "Saved Strava response before decoding");
    Ok(Some(saved))
}

pub(crate) fn exchange_prefix(id: uuid::Uuid) -> String {
    match RequestArchive::current() {
        Some(archive) => format!("{}/exchanges/{id}", archive.key),
        None => format!("strava/exchanges/{id}"),
    }
}

fn strava_error_from_response(
    status: StatusCode,
    headers: &HeaderMap,
    body: &str,
    retry_after: Option<DateTime<Utc>>,
    request_class: StravaRequestClass,
    operation: &'static str,
    now: DateTime<Utc>,
) -> AppError {
    let message = serde_json::from_str::<StravaFault>(body)
        .ok()
        .and_then(|fault| fault.message)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("Strava request failed with status {status}"));

    if status == StatusCode::TOO_MANY_REQUESTS {
        metrics::record_provider_rate_limit_pause(STRAVA_PROVIDER, "remote_429", operation);
        let retry_at = retry_after
            .or_else(|| parse_retry_after_header(headers, now))
            .unwrap_or_else(|| strava_retry_at_for_class_at(request_class, now));
        let current = tracing::Span::current();
        current.record("retry_at", field::display(retry_at));
        current.record("status", "429");
        AppError::too_many_requests(message, Some(retry_at))
    } else if status.is_client_error() {
        AppError::bad_request(message)
    } else {
        AppError::internal(message)
    }
}

fn strava_quota_specs(request_class: StravaRequestClass) -> Vec<ProviderQuotaBucketSpec> {
    let mut specs = vec![
        ProviderQuotaBucketSpec {
            provider: STRAVA_PROVIDER,
            bucket: STRAVA_OVERALL_15_MINUTE_BUCKET,
            limit_count: STRAVA_OVERALL_15_MINUTE_LIMIT,
            window: Duration::minutes(15),
            units: 1,
        },
        ProviderQuotaBucketSpec {
            provider: STRAVA_PROVIDER,
            bucket: STRAVA_OVERALL_DAILY_BUCKET,
            limit_count: STRAVA_OVERALL_DAILY_LIMIT,
            window: Duration::hours(24),
            units: 1,
        },
    ];

    if matches!(request_class, StravaRequestClass::Read) {
        specs.push(ProviderQuotaBucketSpec {
            provider: STRAVA_PROVIDER,
            bucket: STRAVA_READ_15_MINUTE_BUCKET,
            limit_count: STRAVA_READ_15_MINUTE_LIMIT,
            window: Duration::minutes(15),
            units: 1,
        });
        specs.push(ProviderQuotaBucketSpec {
            provider: STRAVA_PROVIDER,
            bucket: STRAVA_READ_DAILY_BUCKET,
            limit_count: STRAVA_READ_DAILY_LIMIT,
            window: Duration::hours(24),
            units: 1,
        });
    }

    specs
}

async fn reconcile_strava_rate_limit_headers(
    db: &DatabaseConnection,
    headers: &HeaderMap,
    request_class: StravaRequestClass,
) {
    let now = Utc::now();
    let short_reset_at = next_strava_short_window_reset(now);
    let daily_reset_at = next_strava_daily_window_reset(now);

    if let (Some((short_limit, daily_limit)), Some((short_used, daily_used))) = (
        parse_strava_rate_limit_pair(headers, "x-ratelimit-limit"),
        parse_strava_rate_limit_pair(headers, "x-ratelimit-usage"),
    ) {
        reconcile_strava_bucket(
            db,
            STRAVA_OVERALL_15_MINUTE_BUCKET,
            short_limit,
            short_used,
            short_reset_at,
        )
        .await;
        reconcile_strava_bucket(
            db,
            STRAVA_OVERALL_DAILY_BUCKET,
            daily_limit,
            daily_used,
            daily_reset_at,
        )
        .await;
    }

    if matches!(request_class, StravaRequestClass::Read) {
        if let (Some((short_limit, daily_limit)), Some((short_used, daily_used))) = (
            parse_strava_rate_limit_pair(headers, "x-readratelimit-limit"),
            parse_strava_rate_limit_pair(headers, "x-readratelimit-usage"),
        ) {
            reconcile_strava_bucket(
                db,
                STRAVA_READ_15_MINUTE_BUCKET,
                short_limit,
                short_used,
                short_reset_at,
            )
            .await;
            reconcile_strava_bucket(
                db,
                STRAVA_READ_DAILY_BUCKET,
                daily_limit,
                daily_used,
                daily_reset_at,
            )
            .await;
        }
    }
}

async fn reconcile_strava_bucket(
    db: &DatabaseConnection,
    bucket: &'static str,
    limit_count: i32,
    used_count: i32,
    reset_at: DateTime<Utc>,
) {
    let span = tracing::info_span!(
        "provider.quota.reconcile",
        provider = STRAVA_PROVIDER,
        bucket,
        limit = limit_count,
        used = used_count,
        reset_at = %reset_at,
        trace_id = field::Empty,
        span_id = field::Empty,
    );

    async move {
        observability::record_current_trace_context();
        if let Err(error) = reconcile_provider_quota_usage(
            db,
            STRAVA_PROVIDER,
            bucket,
            limit_count,
            used_count,
            reset_at,
        )
        .await
        {
            tracing::warn!(
                bucket,
                message = %error.message,
                "failed to reconcile Strava rate-limit headers"
            );
        }
    }
    .instrument(span)
    .await;
}

fn parse_strava_rate_limit_pair(headers: &HeaderMap, header_name: &str) -> Option<(i32, i32)> {
    let value = headers.get(header_name)?.to_str().ok()?;
    let mut parts = value.split(',').map(str::trim);
    let short = parts.next()?.parse::<i32>().ok()?;
    let daily = parts.next()?.parse::<i32>().ok()?;

    if parts.next().is_some() {
        return None;
    }

    Some((short, daily))
}

fn parse_retry_after_header(headers: &HeaderMap, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    if value.is_empty() {
        return None;
    }

    if let Ok(seconds) = value.parse::<i64>() {
        return (seconds >= 0).then_some(now + Duration::seconds(seconds));
    }

    chrono::DateTime::parse_from_rfc2822(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .filter(|timestamp| *timestamp >= now)
}

fn strava_retry_at_for_class_at(
    _request_class: StravaRequestClass,
    now: DateTime<Utc>,
) -> DateTime<Utc> {
    next_strava_short_window_reset(now)
}

fn next_strava_short_window_reset(now: DateTime<Utc>) -> DateTime<Utc> {
    let window_seconds = Duration::minutes(15).num_seconds();
    let next_timestamp = ((now.timestamp() / window_seconds) + 1) * window_seconds;
    DateTime::<Utc>::from_timestamp(next_timestamp, 0).unwrap_or(now + Duration::minutes(15))
}

fn next_strava_daily_window_reset(now: DateTime<Utc>) -> DateTime<Utc> {
    let Some(next_day) = now.date_naive().succ_opt() else {
        return now + Duration::hours(24);
    };
    let Some(midnight) = next_day.and_hms_opt(0, 0, 0) else {
        return now + Duration::hours(24);
    };
    DateTime::<Utc>::from_naive_utc_and_offset(midnight, Utc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_retry_after_seconds_header() {
        let now = DateTime::parse_from_rfc3339("2026-09-12T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, "120".parse().unwrap());

        assert_eq!(
            parse_retry_after_header(&headers, now),
            Some(now + Duration::seconds(120))
        );
    }

    #[test]
    fn parses_retry_after_http_date_header() {
        let now = DateTime::parse_from_rfc3339("2026-09-12T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let retry_at = DateTime::parse_from_rfc3339("2026-09-12T12:03:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut headers = HeaderMap::new();
        headers.insert(
            RETRY_AFTER,
            "Sat, 12 Sep 2026 12:03:00 GMT".parse().unwrap(),
        );

        assert_eq!(parse_retry_after_header(&headers, now), Some(retry_at));
    }

    #[test]
    fn strava_429_error_uses_retry_after_header() {
        let now = DateTime::parse_from_rfc3339("2026-09-12T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let retry_at = now + Duration::seconds(180);
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, "180".parse().unwrap());

        let error = strava_error_from_response(
            StatusCode::TOO_MANY_REQUESTS,
            &headers,
            r#"{"message":"Rate Limit Exceeded"}"#,
            None,
            StravaRequestClass::Read,
            "get_activity_streams",
            now,
        );

        assert_eq!(error.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(error.message, "Rate Limit Exceeded");
        assert_eq!(error.retry_at, Some(retry_at));
    }

    #[test]
    fn strava_429_error_falls_back_to_next_short_window() {
        let now = DateTime::parse_from_rfc3339("2026-09-12T12:07:30Z")
            .unwrap()
            .with_timezone(&Utc);
        let retry_at = DateTime::parse_from_rfc3339("2026-09-12T12:15:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let headers = HeaderMap::new();

        let error = strava_error_from_response(
            StatusCode::TOO_MANY_REQUESTS,
            &headers,
            "{}",
            None,
            StravaRequestClass::Read,
            "list_activities",
            now,
        );

        assert_eq!(error.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            error.message,
            "Strava request failed with status 429 Too Many Requests"
        );
        assert_eq!(error.retry_at, Some(retry_at));
    }

    #[test]
    fn parses_refresh_token_response_without_scope_or_athlete() {
        let response = serde_json::from_str::<StravaRefreshTokenResponse>(
            r#"{
                "access_token": "refreshed-access",
                "expires_at": 1760000000,
                "expires_in": 21600,
                "refresh_token": "refreshed-refresh"
            }"#,
        )
        .expect("parse refresh response");

        assert_eq!(response.access_token, "refreshed-access");
        assert_eq!(response.refresh_token, "refreshed-refresh");
        assert_eq!(response.expires_at, 1_760_000_000);
        assert_eq!(response.scope, None);
        assert!(response.athlete.is_none());
    }
}
