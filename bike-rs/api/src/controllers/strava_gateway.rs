use crate::app_error::AppError;
use crate::storage::AppStorage;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use bike_core::background_jobs::pipeline::PipelineContext;
use bike_core::config::Config;
use bike_core::strava_gateway_delivery::{
    intent::{self, DeliverySource},
    GatewayPayload,
};
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
    #[serde(default)]
    pipeline: Option<PipelineContext>,
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
    let context = envelope.pipeline.clone().or_else(PipelineContext::current);
    let source = envelope.into_source()?;
    if let Some(carrier) = context
        .as_ref()
        .and_then(|context| context.trace_context.as_ref())
    {
        bike_core::observability::set_span_parent_from_carrier(
            &tracing::Span::current(),
            Some(carrier),
        );
    }
    let accepted = PipelineContext::scope(context, intent::accept(&state.db, source)).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({"status":if accepted{"queued"}else{"already_accepted"}})),
    ))
}

impl Envelope {
    /// Validate the signed provider content before handing its owned source to
    /// the durable queue. Parsing here does not execute the ingestion workflow.
    fn into_source(self) -> Result<DeliverySource, AppError> {
        let payload = if self.operation == "upsert" {
            let raw = self
                .payload
                .as_ref()
                .ok_or_else(|| AppError::bad_request("Missing Strava activity payload"))?;
            let actual = Sha256::digest(raw.get().as_bytes());
            let expected = self
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
        if let Some(payload) = &payload {
            if payload.activity.id != self.activity_id {
                return Err(AppError::bad_request(
                    "Strava activity ID does not match delivery",
                ));
            }
        }
        drop(payload);
        Ok(DeliverySource {
            delivery_id: self.delivery_id,
            athlete_id: self.athlete_id,
            user_id: i32::try_from(self.site_user_id)
                .map_err(|_| AppError::bad_request("Invalid Bike user ID"))?,
            activity_id: self.activity_id,
            event_time: self.event_time,
            operation: self.operation,
            payload: self
                .payload
                .map(|raw| serde_json::from_str(raw.get()))
                .transpose()
                .map_err(|_| AppError::bad_request("Invalid source"))?,
        })
    }
}

fn validate_envelope(envelope: &Envelope, headers: &HeaderMap) -> Result<(), AppError> {
    if let Some(origin) = &envelope.pipeline {
        if uuid::Uuid::parse_str(&origin.run_id).is_err()
            || !["strava_webhook", "strava_sync"].contains(&origin.entrypoint.as_str())
            || origin.parent_task_id.is_some()
            || origin.pipeline_started_at.timestamp() <= 0
            || origin.pipeline_started_at > Utc::now() + chrono::Duration::minutes(5)
        {
            return Err(AppError::bad_request("Invalid signed pipeline origin"));
        }
    }
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
    use super::*;
    use axum::http::HeaderMap;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    fn envelope() -> Envelope {
        serde_json::from_value(json!({
            "version":1,"delivery_id":"rust:3","athlete_id":7,"site_user_id":7,
            "activity_id":42,"event_time":1,"operation":"delete",
            "pipeline":PipelineContext::received("strava_webhook",Some("request".into())),
        }))
        .unwrap()
    }

    #[test]
    fn signed_origins_reject_forged_parents_entrypoints_and_receipt_clocks() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Bike-Delivery-ID", "rust:3".parse().unwrap());
        validate_envelope(&envelope(), &headers).unwrap();
        for change in ["parent", "entrypoint", "clock"] {
            let mut received = envelope();
            let origin = received.pipeline.as_mut().unwrap();
            match change {
                "parent" => origin.parent_task_id = Some(1),
                "entrypoint" => origin.entrypoint = "untrusted".into(),
                _ => origin.pipeline_started_at = chrono::DateTime::from_timestamp(0, 0).unwrap(),
            }
            assert!(validate_envelope(&received, &headers).is_err());
        }
    }

    #[test]
    fn owned_source_validates_provider_hash_and_identity_before_queueing() {
        let deleted = envelope().into_source().unwrap();
        assert_eq!(deleted.operation, "delete");
        assert!(deleted.payload.is_none());
        let value =
            json!({"activity":{"id":42,"name":"Fixture ride","start_date":"2026-10-10T12:00:00Z"}});
        let raw = value.to_string();
        let mut upsert = envelope();
        upsert.operation = "upsert".into();
        upsert.content_sha256 = Some(hex::encode(Sha256::digest(raw.as_bytes())));
        upsert.payload = Some(RawValue::from_string(raw).unwrap());
        assert_eq!(upsert.into_source().unwrap().payload, Some(value));
        let mut bad = envelope();
        bad.operation = "upsert".into();
        assert!(bad.into_source().is_err());
    }

    #[test]
    fn owned_source_rejects_tampered_hash_mismatched_activity_and_invalid_payload() {
        for kind in ["hash", "identity", "payload", "encoding"] {
            let raw = if kind == "payload" { json!({"unexpected":true}) } else {
                json!({"activity":{"id":42,"name":"Fixture ride","start_date":"2026-10-10T12:00:00Z"}})
            }.to_string();
            let mut received = envelope();
            received.operation = "upsert".into();
            received.content_sha256 = Some(hex::encode(Sha256::digest(raw.as_bytes())));
            received.payload = Some(RawValue::from_string(raw).unwrap());
            match kind {
                "hash" => received.content_sha256 = Some("00".repeat(32)),
                "identity" => received.activity_id = 43,
                "encoding" => received.content_sha256 = Some("invalid".into()),
                _ => {}
            }
            assert!(received.into_source().is_err(), "accepted invalid {kind}");
        }
    }

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
