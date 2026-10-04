use crate::strava_gateway_client::GatewayConnection;
use crate::workflow_error::WorkflowError as AppError;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use tonic::transport::{Channel, Endpoint};

pub mod proto {
    tonic::include_proto!("bike.strava.v1");
}

type HmacSha256 = Hmac<Sha256>;

pub struct GatewayGrpcClient {
    client: proto::gateway_service_client::GatewayServiceClient<Channel>,
    secret: String,
}

impl GatewayGrpcClient {
    pub fn new(address: &str, secret: &str) -> Result<Self, AppError> {
        let endpoint = Endpoint::from_shared(address.to_string())
            .map_err(|error| {
                AppError::internal(format!("Invalid Strava gateway gRPC address: {error}"))
            })?
            .timeout(std::time::Duration::from_secs(5));
        Ok(Self {
            client: proto::gateway_service_client::GatewayServiceClient::new(
                endpoint.connect_lazy(),
            ),
            secret: secret.to_string(),
        })
    }

    pub async fn begin_connect(&self, user_id: i32) -> Result<String, AppError> {
        let request = self.signed(
            "BeginConnect",
            user_id,
            "",
            proto::SiteRequest {
                target: "rust".to_string(),
                site_user_id: i64::from(user_id),
            },
        )?;
        let response = self
            .client
            .clone()
            .begin_connect(request)
            .await
            .map_err(unavailable)?
            .into_inner();
        if response.authorization_url.is_empty() {
            return Err(AppError::internal(
                "Strava gateway returned no authorization URL",
            ));
        }
        Ok(response.authorization_url)
    }

    pub async fn connection(&self, user_id: i32) -> Result<GatewayConnection, AppError> {
        let request = self.signed(
            "GetConnection",
            user_id,
            "",
            proto::SiteRequest {
                target: "rust".to_string(),
                site_user_id: i64::from(user_id),
            },
        )?;
        let response = self
            .client
            .clone()
            .get_connection(request)
            .await
            .map_err(unavailable)?
            .into_inner();
        let last_sync_next_attempt_at = response
            .last_sync_next_attempt_at
            .map(|value| {
                chrono::DateTime::parse_from_rfc3339(&value)
                    .map(|time| time.with_timezone(&chrono::Utc))
                    .map_err(|_| {
                        AppError::internal("Strava gateway returned an invalid sync retry time")
                    })
            })
            .transpose()?;
        Ok(GatewayConnection {
            configured: response.configured,
            connected: response.connected,
            athlete_id: response.athlete_id,
            scopes: response.scopes,
            last_sync_status: response.last_sync_status,
            last_sync_wait_reason: response.last_sync_wait_reason,
            last_sync_next_attempt_at,
            last_sync_imported_count: response.last_sync_imported_count,
            last_sync_duplicate_count: response.last_sync_duplicate_count,
            last_sync_failed_count: response.last_sync_failed_count,
        })
    }

    pub async fn queue_sync(&self, user_id: i32) -> Result<(), AppError> {
        let mode = "incremental";
        let request = self.signed(
            "QueueSync",
            user_id,
            mode,
            proto::SyncRequest {
                target: "rust".to_string(),
                site_user_id: i64::from(user_id),
                mode: mode.to_string(),
            },
        )?;
        self.client
            .clone()
            .queue_sync(request)
            .await
            .map_err(unavailable)?;
        Ok(())
    }

    pub async fn disconnect(&self, user_id: i32) -> Result<(), AppError> {
        let request = self.signed(
            "Disconnect",
            user_id,
            "",
            proto::SiteRequest {
                target: "rust".to_string(),
                site_user_id: i64::from(user_id),
            },
        )?;
        self.client
            .clone()
            .disconnect(request)
            .await
            .map_err(unavailable)?;
        Ok(())
    }

    fn signed<T>(
        &self,
        method: &str,
        user_id: i32,
        mode: &str,
        body: T,
    ) -> Result<tonic::Request<T>, AppError> {
        if user_id <= 0 {
            return Err(AppError::bad_request("Invalid Bike user ID"));
        }
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let message = format!(
            "{timestamp}\n/bike.strava.v1.GatewayService/{method}\nrust\n{user_id}\n{mode}"
        );
        let mut mac = HmacSha256::new_from_slice(self.secret.as_bytes())
            .map_err(|_| AppError::internal("Invalid Strava gateway shared secret"))?;
        mac.update(message.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());
        let mut request = tonic::Request::new(body);
        request.metadata_mut().insert(
            "x-bike-timestamp",
            timestamp
                .parse()
                .map_err(|_| AppError::internal("Invalid Strava gateway timestamp"))?,
        );
        request.metadata_mut().insert(
            "x-bike-signature",
            signature
                .parse()
                .map_err(|_| AppError::internal("Invalid Strava gateway signature"))?,
        );
        if let Some(carrier) = crate::observability::inject_current_trace_context() {
            if let Some(value) = carrier.get("traceparent") {
                request.metadata_mut().insert(
                    "traceparent",
                    value
                        .parse()
                        .map_err(|_| AppError::internal("Invalid traceparent metadata"))?,
                );
            }
            if let Some(value) = carrier.get("tracestate") {
                request.metadata_mut().insert(
                    "tracestate",
                    value
                        .parse()
                        .map_err(|_| AppError::internal("Invalid tracestate metadata"))?,
                );
            }
        }
        Ok(request)
    }
}

fn unavailable(error: tonic::Status) -> AppError {
    AppError::internal(format!("Strava gateway unavailable: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{proto, GatewayGrpcClient, HmacSha256, Mac};

    #[tokio::test]
    async fn site_signature_binds_grpc_method_and_user() {
        let client = GatewayGrpcClient::new("http://127.0.0.1:50051", "shared-secret").unwrap();
        let request = client
            .signed(
                "GetConnection",
                7,
                "",
                proto::SiteRequest {
                    target: "rust".to_string(),
                    site_user_id: 7,
                },
            )
            .unwrap();
        let timestamp = request
            .metadata()
            .get("x-bike-timestamp")
            .unwrap()
            .to_str()
            .unwrap();
        let signature = request
            .metadata()
            .get("x-bike-signature")
            .unwrap()
            .to_str()
            .unwrap();
        let mut mac = HmacSha256::new_from_slice(b"shared-secret").unwrap();
        mac.update(
            format!("{timestamp}\n/bike.strava.v1.GatewayService/GetConnection\nrust\n7\n")
                .as_bytes(),
        );
        assert_eq!(signature, hex::encode(mac.finalize().into_bytes()));
    }
}
