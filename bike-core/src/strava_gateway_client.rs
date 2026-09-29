use crate::config::Config;
use crate::workflow_error::WorkflowError as AppError;
use hmac::{Hmac, Mac};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::time::Duration;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Deserialize)]
pub struct GatewayConnection {
    pub configured: bool,
    pub connected: bool,
    pub athlete_id: Option<i64>,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub last_sync_status: String,
    pub last_sync_imported_count: i32,
    pub last_sync_duplicate_count: i32,
    pub last_sync_failed_count: i32,
}

#[derive(Serialize)]
struct SiteRequest<'a> {
    target: &'static str,
    site_user_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<&'a str>,
}

#[derive(Deserialize)]
struct AuthorizationResponse {
    authorization_url: String,
}

pub struct StravaGatewayClient {
    base_url: String,
    shared_secret: String,
    http: reqwest::Client,
}

impl StravaGatewayClient {
    pub fn from_config(config: &Config) -> Result<Self, AppError> {
        let base_url = config
            .strava_gateway_url
            .as_ref()
            .ok_or_else(|| AppError::internal("Strava gateway is not configured"))?;
        if config.strava_gateway_shared_secret.is_empty() {
            return Err(AppError::internal(
                "Strava gateway shared secret is missing",
            ));
        }
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|error| AppError::internal(format!("Strava gateway client: {error}")))?;
        Ok(Self {
            base_url: base_url.clone(),
            shared_secret: config.strava_gateway_shared_secret.clone(),
            http,
        })
    }

    pub async fn begin_connect(&self, user_id: i32) -> Result<String, AppError> {
        let body = self.post("/v1/oauth/intents", user_id, None).await?;
        let response: AuthorizationResponse = serde_json::from_slice(&body)
            .map_err(|_| AppError::internal("Invalid Strava gateway authorization response"))?;
        if response.authorization_url.is_empty() {
            return Err(AppError::internal(
                "Strava gateway returned no authorization URL",
            ));
        }
        Ok(response.authorization_url)
    }

    pub async fn connection(&self, user_id: i32) -> Result<GatewayConnection, AppError> {
        let body = self.post("/v1/connections/status", user_id, None).await?;
        serde_json::from_slice(&body)
            .map_err(|_| AppError::internal("Invalid Strava gateway connection response"))
    }

    pub async fn queue_sync(&self, user_id: i32) -> Result<(), AppError> {
        self.post("/v1/sync", user_id, Some("incremental"))
            .await
            .map(|_| ())
    }

    pub async fn disconnect(&self, user_id: i32) -> Result<(), AppError> {
        self.post("/v1/connections/disconnect", user_id, None)
            .await
            .map(|_| ())
    }

    async fn post(
        &self,
        path: &str,
        user_id: i32,
        mode: Option<&str>,
    ) -> Result<Vec<u8>, AppError> {
        if user_id <= 0 {
            return Err(AppError::bad_request("Invalid Bike user ID"));
        }
        let body = serde_json::to_vec(&SiteRequest {
            target: "rust",
            site_user_id: user_id,
            mode,
        })
        .map_err(|error| AppError::internal(format!("Strava gateway request: {error}")))?;
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let signature = sign_request(&self.shared_secret, &timestamp, path, &body)?;
        let response = self
            .http
            .post(format!("{}{path}", self.base_url))
            .header("X-Bike-Request-Timestamp", timestamp)
            .header("X-Bike-Request-Signature", signature)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|error| AppError::internal(format!("Strava gateway unavailable: {error}")))?;
        if response.status() != StatusCode::OK {
            return Err(match response.status() {
                StatusCode::BAD_REQUEST => AppError::bad_request("Invalid Strava gateway request"),
                StatusCode::NOT_FOUND => AppError::not_found("No Strava connection exists"),
                StatusCode::CONFLICT => AppError::conflict(
                    "Strava gateway request conflicts with the current connection",
                ),
                StatusCode::TOO_MANY_REQUESTS => {
                    AppError::too_many_requests("Strava gateway is rate limited", None)
                }
                status => AppError::internal(format!("Strava gateway returned HTTP {status}")),
            });
        }
        response
            .bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|error| AppError::internal(format!("Strava gateway response: {error}")))
    }
}

fn sign_request(
    secret: &str,
    timestamp: &str,
    path: &str,
    body: &[u8],
) -> Result<String, AppError> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| AppError::internal("Invalid Strava gateway shared secret"))?;
    mac.update(format!("{timestamp}\nPOST {path}\n").as_bytes());
    mac.update(body);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::sign_request;

    #[test]
    fn signs_site_requests_with_exact_path_and_body() {
        let body = br#"{"target":"rust","site_user_id":7}"#;
        assert_eq!(
            sign_request("secret", "123", "/v1/oauth/intents", body).unwrap(),
            "1760f2a1e2d9e8ad2dddad2d411112a197c7055b3dbef80d2707f147195dceb4"
        );
    }
}
