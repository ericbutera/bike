use crate::app_error::AppError;
use crate::config::Config;
use crate::metrics;
use crate::observability;
use crate::provider_rate_limit::{
    reconcile_provider_quota_usage, reserve_provider_quota, ProviderQuotaBucketSpec,
    ProviderQuotaReservation,
};
use crate::strava_provider_payload::{StravaActivityStreams, StravaActivitySummary};
use axum::http::StatusCode;
use chrono::{DateTime, Duration, Utc};
use reqwest::header::{HeaderMap, RETRY_AFTER};
use reqwest::{Client, RequestBuilder};
use sea_orm::DatabaseConnection;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use tracing::field;
use tracing::Instrument;

pub(crate) const STRAVA_TOKEN_URL: &str = "https://www.strava.com/api/v3/oauth/token";
pub(crate) const STRAVA_DEAUTHORIZE_URL: &str = "https://www.strava.com/oauth/deauthorize";
pub(crate) const STRAVA_API_BASE_URL: &str = "https://www.strava.com/api/v3";
pub(crate) const STRAVA_PUSH_SUBSCRIPTIONS_URL: &str =
    "https://www.strava.com/api/v3/push_subscriptions";
const STRAVA_PROVIDER: &str = "strava";
const STRAVA_OVERALL_15_MINUTE_BUCKET: &str = "overall_15_minute";
const STRAVA_OVERALL_DAILY_BUCKET: &str = "overall_daily";
const STRAVA_READ_15_MINUTE_BUCKET: &str = "read_15_minute";
const STRAVA_READ_DAILY_BUCKET: &str = "read_daily";
const STRAVA_OVERALL_15_MINUTE_LIMIT: i32 = 400;
const STRAVA_OVERALL_DAILY_LIMIT: i32 = 4_000;
const STRAVA_READ_15_MINUTE_LIMIT: i32 = 200;
const STRAVA_READ_DAILY_LIMIT: i32 = 2_000;

pub(crate) struct StravaApiClient {
    client: Client,
    db: DatabaseConnection,
    config: &'static Config,
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
pub(crate) struct StravaPushSubscription {
    pub(crate) id: i64,
    pub(crate) callback_url: String,
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
            client,
            db: db.clone(),
            config,
        })
    }

    pub(crate) async fn exchange_authorization_code(
        &self,
        code: &str,
    ) -> Result<StravaAuthorizationTokenResponse, AppError> {
        self.send_json(
            self.client.post(STRAVA_TOKEN_URL).form(&[
                ("client_id", self.config.strava_client_id.as_str()),
                ("client_secret", self.config.strava_client_secret.as_str()),
                ("code", code),
                ("grant_type", "authorization_code"),
            ]),
            StravaRequestClass::OverallOnly,
            "exchange_authorization_code",
            "exchange a Strava authorization code",
        )
        .await
    }

    pub(crate) async fn refresh_access_token(
        &self,
        refresh_token: &str,
    ) -> Result<StravaRefreshTokenResponse, AppError> {
        self.send_json(
            self.client.post(STRAVA_TOKEN_URL).form(&[
                ("client_id", self.config.strava_client_id.as_str()),
                ("client_secret", self.config.strava_client_secret.as_str()),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),
            ]),
            StravaRequestClass::OverallOnly,
            "refresh_access_token",
            "refresh a Strava access token",
        )
        .await
    }

    pub(crate) async fn list_activities(
        &self,
        access_token: &str,
        after_epoch: Option<i64>,
        page: usize,
        per_page: usize,
    ) -> Result<Vec<StravaActivitySummary>, AppError> {
        let mut request = self
            .client
            .get(format!("{STRAVA_API_BASE_URL}/athlete/activities"))
            .bearer_auth(access_token)
            .query(&[("page", page), ("per_page", per_page)]);

        if let Some(after_epoch) = after_epoch {
            request = request.query(&[("after", after_epoch)]);
        }

        self.send_json(
            request,
            StravaRequestClass::Read,
            "list_activities",
            "list Strava activities",
        )
        .await
    }

    pub(crate) async fn get_activity_streams(
        &self,
        access_token: &str,
        activity_id: i64,
    ) -> Result<StravaActivityStreams, AppError> {
        self.send_json(
            self.client
                .get(format!(
                    "{STRAVA_API_BASE_URL}/activities/{activity_id}/streams"
                ))
                .bearer_auth(access_token)
                .query(&[
                    (
                        "keys",
                        "time,distance,latlng,altitude,velocity_smooth,heartrate,cadence,watts,temp,moving,grade_smooth",
                    ),
                    ("key_by_type", "true"),
                ]),
            StravaRequestClass::Read,
            "get_activity_streams",
            "fetch Strava activity streams",
        )
        .await
    }

    pub(crate) async fn deauthorize(&self, access_token: &str) -> Result<(), AppError> {
        self.send_json::<serde_json::Value>(
            self.client
                .post(STRAVA_DEAUTHORIZE_URL)
                .query(&[("access_token", access_token)]),
            StravaRequestClass::OverallOnly,
            "deauthorize",
            "deauthorize the Strava app",
        )
        .await
        .map(|_| ())
    }

    pub(crate) async fn list_push_subscriptions(
        &self,
    ) -> Result<Vec<StravaPushSubscription>, AppError> {
        self.send_json(
            self.client.get(STRAVA_PUSH_SUBSCRIPTIONS_URL).query(&[
                ("client_id", self.config.strava_client_id.as_str()),
                ("client_secret", self.config.strava_client_secret.as_str()),
            ]),
            StravaRequestClass::Read,
            "list_push_subscriptions",
            "list Strava webhook subscriptions",
        )
        .await
    }

    pub(crate) async fn create_push_subscription(
        &self,
    ) -> Result<StravaPushSubscription, AppError> {
        let callback_url = self.config.strava_webhook_callback_url();
        self.send_json(
            self.client.post(STRAVA_PUSH_SUBSCRIPTIONS_URL).form(&[
                ("client_id", self.config.strava_client_id.as_str()),
                ("client_secret", self.config.strava_client_secret.as_str()),
                ("callback_url", callback_url.as_str()),
                (
                    "verify_token",
                    self.config.strava_webhook_verify_token.as_str(),
                ),
            ]),
            StravaRequestClass::OverallOnly,
            "create_push_subscription",
            "create a Strava webhook subscription",
        )
        .await
    }

    async fn send_json<T>(
        &self,
        request: RequestBuilder,
        request_class: StravaRequestClass,
        operation: &'static str,
        action: &str,
    ) -> Result<T, AppError>
    where
        T: DeserializeOwned,
    {
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
                request.send().await
            }
            .instrument(http_span.clone())
            .await;
            observability::record_span_trace_context(&http_span);
            match &response {
                Ok(response) => {
                    let status_label = response.status().as_u16().to_string();
                    http_span.record("status", field::display(&status_label));
                    if let Some(retry_at) = parse_retry_after_header(response.headers(), Utc::now())
                    {
                        http_span.record("retry_at", field::display(retry_at));
                    }
                }
                Err(_) => {
                    http_span.record("status", "transport_error");
                }
            }
            self.parse_json_response(response, request_class, operation, action)
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
        response: Result<reqwest::Response, reqwest::Error>,
        request_class: StravaRequestClass,
        operation: &'static str,
        action: &str,
    ) -> Result<T, AppError>
    where
        T: DeserializeOwned,
    {
        let response = response.map_err(|error| {
            tracing::Span::current().record("status", "transport_error");
            metrics::record_provider_api_request(
                STRAVA_PROVIDER,
                operation,
                request_class.as_str(),
                "transport_error",
            );
            tracing::error!(error = ?error, action, operation, "Strava request failed");
            AppError::internal(format!("Failed to {action}"))
        })?;
        let status = response.status();
        let status_label = status.as_u16().to_string();
        tracing::Span::current().record("status", field::display(&status_label));
        metrics::record_provider_api_request(
            STRAVA_PROVIDER,
            operation,
            request_class.as_str(),
            &status_label,
        );
        reconcile_strava_rate_limit_headers(&self.db, response.headers(), request_class).await;
        let retry_after = parse_retry_after_header(response.headers(), Utc::now());

        if !status.is_success() {
            let headers = response.headers().clone();
            let body = response.text().await.unwrap_or_default();
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

        response.json::<T>().await.map_err(|error| {
            tracing::error!(error = ?error, action, "failed to parse Strava response");
            AppError::internal(format!(
                "Failed to parse Strava response while trying to {action}"
            ))
        })
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
