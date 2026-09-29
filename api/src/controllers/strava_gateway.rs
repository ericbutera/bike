use crate::app_error::AppError;
use crate::storage::AppStorage;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use bike_core::config::Config;
use bike_core::entities::strava_gateway::{Claim, Receipt};
use bike_core::strava_gateway_delivery::{self, GatewayPayload};
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{json, value::RawValue};
use sha2::{Digest, Sha256};
use std::sync::Arc;

type HmacSha256 = Hmac<Sha256>;
const MAX_DELIVERY_BYTES: usize = 34 * 1024 * 1024;

#[derive(Deserialize)]
struct Envelope {
    version: i32,
    delivery_id: String,
    athlete_id: i64,
    site_user_id: i64,
    activity_id: i64,
    event_time: i64,
    operation: String,
    content_sha256: Option<String>,
    payload: Option<Box<RawValue>>,
}

pub fn routes() -> Router<Arc<AppStorage>> {
    Router::new().route(
        "/internal/strava-deliveries",
        post(deliver).layer(DefaultBodyLimit::max(MAX_DELIVERY_BYTES)),
    )
}

async fn deliver(
    State(state): State<Arc<AppStorage>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let secret = Config::get().strava_gateway_shared_secret.as_str();
    if secret.is_empty() {
        return Err(AppError::internal(
            "Strava gateway receiver is not configured",
        ));
    }
    if !valid_signature(&headers, &body, secret, Utc::now().timestamp()) {
        return Err(AppError::unauthorized(
            "Invalid Strava gateway delivery signature",
        ));
    }
    let envelope: Envelope = serde_json::from_slice(&body)
        .map_err(|_| AppError::bad_request("Invalid Strava gateway delivery"))?;
    validate_envelope(&envelope, &headers)?;
    let payload = if envelope.operation == "upsert" {
        let raw = envelope
            .payload
            .as_ref()
            .ok_or_else(|| AppError::bad_request("Missing Strava activity payload"))?;
        let actual = Sha256::digest(raw.get().as_bytes());
        let expected = envelope
            .content_sha256
            .as_deref()
            .and_then(|value| hex::decode(value).ok())
            .ok_or_else(|| AppError::bad_request("Invalid Strava activity hash"))?;
        if expected.as_slice() != actual.as_slice() {
            return Err(AppError::bad_request(
                "Strava activity hash does not match payload",
            ));
        }
        Some(
            serde_json::from_str::<GatewayPayload>(raw.get())
                .map_err(|_| AppError::bad_request("Invalid Strava activity payload"))?,
        )
    } else {
        None
    };
    let receipt = Receipt {
        delivery_id: &envelope.delivery_id,
        athlete_id: envelope.athlete_id,
        user_id: i32::try_from(envelope.site_user_id)
            .map_err(|_| AppError::bad_request("Invalid Bike user ID"))?,
        activity_id: envelope.activity_id,
        event_time: envelope.event_time,
        operation: &envelope.operation,
    };
    match strava_gateway_delivery::receive(
        &state.db,
        &state.tasks,
        &state.uploads_dir,
        &receipt,
        payload,
    )
    .await?
    {
        Claim::Acquired => Ok((StatusCode::OK, Json(json!({"status":"applied"})))),
        Claim::Completed => Ok((StatusCode::OK, Json(json!({"status":"already_applied"})))),
        Claim::Busy => Err(AppError::internal("Strava delivery is already processing")),
    }
}

fn validate_envelope(envelope: &Envelope, headers: &HeaderMap) -> Result<(), AppError> {
    if envelope.version != 1
        || envelope.athlete_id <= 0
        || envelope.site_user_id <= 0
        || envelope.site_user_id > i64::from(i32::MAX)
        || envelope.activity_id <= 0
        || envelope.event_time <= 0
        || !envelope.delivery_id.starts_with("rust:")
        || !matches!(
            envelope.operation.as_str(),
            "upsert" | "delete" | "deauthorize"
        )
        || header(headers, "X-Bike-Delivery-ID") != Some(envelope.delivery_id.as_str())
    {
        return Err(AppError::bad_request("Invalid Strava gateway delivery"));
    }
    Ok(())
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

fn valid_signature(headers: &HeaderMap, body: &[u8], secret: &str, now: i64) -> bool {
    let Some(timestamp) = header(headers, "X-Bike-Delivery-Timestamp") else {
        return false;
    };
    let Ok(seconds) = timestamp.parse::<i64>() else {
        return false;
    };
    if now.abs_diff(seconds) > 300 {
        return false;
    }
    let Some(id) = header(headers, "X-Bike-Delivery-ID") else {
        return false;
    };
    let Some(signature) =
        header(headers, "X-Bike-Delivery-Signature").and_then(|value| hex::decode(value).ok())
    else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(format!("{timestamp}\n{id}\n").as_bytes());
    mac.update(body);
    mac.verify_slice(&signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::valid_signature;
    use axum::http::HeaderMap;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    #[test]
    fn signed_delivery_accepts_exact_body_and_rejects_tampering() {
        let body = br#"{"version":1,"delivery_id":"rust:3"}"#;
        let mut headers = HeaderMap::new();
        headers.insert("X-Bike-Delivery-Timestamp", "1000".parse().unwrap());
        headers.insert("X-Bike-Delivery-ID", "rust:3".parse().unwrap());
        let mut mac = Hmac::<Sha256>::new_from_slice(b"secret").unwrap();
        mac.update(b"1000\nrust:3\n");
        mac.update(body);
        headers.insert(
            "X-Bike-Delivery-Signature",
            hex::encode(mac.finalize().into_bytes()).parse().unwrap(),
        );
        assert!(valid_signature(&headers, body, "secret", 1000));
        assert!(!valid_signature(&headers, b"{}", "secret", 1000));
        assert!(!valid_signature(&headers, body, "secret", 1301));
        assert!(!valid_signature(&headers, body, "wrong", 1000));
    }
}
