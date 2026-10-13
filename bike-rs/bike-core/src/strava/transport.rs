//! Supported SDK middleware: retain the exchange without changing the request.
use crate::strava::{client::retain_response, credentials, objects::ObjectStore};
use axum::{
    body::Bytes,
    http::{self, HeaderMap, StatusCode},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use http_body_util::BodyExt;
use reqwest_middleware::{Middleware, Next};
use std::cell::RefCell;

#[derive(Clone)]
pub(crate) struct RawResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub bytes: Bytes,
}

tokio::task_local! { static RESPONSE: RefCell<Option<RawResponse>>; }

pub(crate) async fn capture<F: std::future::Future>(future: F) -> (F::Output, Option<RawResponse>) {
    RESPONSE
        .scope(RefCell::new(None), async {
            let result = future.await;
            let response = RESPONSE.with(|slot| slot.borrow_mut().take());
            (result, response)
        })
        .await
}

pub(crate) struct ExchangeRecorder {
    pub store: ObjectStore,
}

#[async_trait::async_trait]
impl Middleware for ExchangeRecorder {
    async fn handle(
        &self,
        request: reqwest::Request,
        extensions: &mut http::Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<reqwest::Response> {
        let id = uuid::Uuid::new_v4();
        let prefix = crate::strava::client::exchange_prefix(id);
        let confidential = request.url().path().starts_with("/oauth/");
        let request_evidence = serde_json::json!({
            "method":request.method().as_str(), "url":request.url().as_str(),
            "headers_base64":headers(request.headers()),
            "body_base64":request.body().and_then(reqwest::Body::as_bytes).map(|body| STANDARD.encode(body)),
            "observed_at":chrono::Utc::now()
        });
        let request_object = self
            .encrypted(&format!("{prefix}/request"), &request_evidence)
            .await?;
        tracing::info!(method = %request.method(), path = request.url().path(), request_object = %request_object.path, "Sending generated Strava SDK request");
        let response = match next.run(request, extensions).await {
            Ok(response) => response,
            Err(error) => {
                self.encrypted(&format!("{prefix}/result"), &serde_json::json!({"request":request_object,"outcome":"transport_error","observed_at":chrono::Utc::now()})).await?;
                return Err(error);
            }
        };
        // Preserve status, version, all headers, URL/extensions, and body bytes.
        // The generated SDK receives the same response for its normal decoding.
        let response: http::Response<reqwest::Body> = response.into();
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await?.to_bytes();
        let raw = RawResponse {
            status: parts.status,
            headers: parts.headers.clone(),
            bytes: bytes.clone(),
        };
        self.encrypted(&format!("{prefix}/result"), &serde_json::json!({
            "request":request_object,"status":raw.status.as_u16(),"version":format!("{:?}",parts.version),
            "headers_base64":headers(&raw.headers),"body_base64":STANDARD.encode(&bytes),"observed_at":chrono::Utc::now()
        })).await?;
        retain_response(&self.store, &prefix, &raw, confidential)
            .await
            .map_err(storage_error)?;
        RESPONSE
            .try_with(|slot| *slot.borrow_mut() = Some(raw))
            .map_err(|_| {
                reqwest_middleware::Error::Middleware(anyhow::anyhow!(
                    "Strava SDK call has no capture context"
                ))
            })?;
        Ok(http::Response::from_parts(parts, reqwest::Body::from(bytes)).into())
    }
}

impl ExchangeRecorder {
    async fn encrypted(
        &self,
        prefix: &str,
        value: &serde_json::Value,
    ) -> reqwest_middleware::Result<crate::strava::objects::ObjectRef> {
        let bytes = serde_json::to_vec(value)
            .map_err(|error| reqwest_middleware::Error::Middleware(error.into()))?;
        let encrypted = credentials::seal(&bytes).map_err(storage_error)?;
        self.store
            .write(prefix, encrypted.as_bytes())
            .await
            .map_err(storage_error)
    }
}

fn headers(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| (name.to_string(), STANDARD.encode(value.as_bytes())))
        .collect()
}

fn storage_error(error: crate::workflow_error::WorkflowError) -> reqwest_middleware::Error {
    reqwest_middleware::Error::Middleware(anyhow::anyhow!(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, Response},
        routing::{get, post},
        Router,
    };
    use strava_sdk::apis::{activities_api, configuration::Configuration, strava_api};

    const ACTIVITY: &[u8] = b" \n{\"id\":20424619693,\"name\":\"Ride\",\"unmodeled\":{\"precision\":123.456789012345}}\n";
    const OAUTH: &[u8] = b" {\"access_token\":\"vendor-secret\",\"refresh_token\":\"rotated-secret\",\"expires_at\":1999999999,\"extra\":true}\n";

    async fn activity(request: Request<Body>) -> Response<Body> {
        assert_eq!(request.headers()["authorization"], "Bearer fixture-access");
        assert_eq!(request.uri().query(), Some("include_all_efforts=false"));
        Response::builder()
            .header("content-type", "application/json")
            .header("x-vendor-request", "original-response")
            .body(Body::from(ACTIVITY))
            .unwrap()
    }

    async fn oauth(request: Request<Body>) -> Response<Body> {
        assert_eq!(
            request.headers()["content-type"],
            "application/x-www-form-urlencoded"
        );
        let body = to_bytes(request.into_body(), 4096).await.unwrap();
        let fields: std::collections::BTreeMap<_, _> =
            url::form_urlencoded::parse(&body).into_owned().collect();
        assert_eq!(fields["client_id"], "fixture-client");
        assert_eq!(fields["client_secret"], "fixture-secret");
        assert_eq!(fields["grant_type"], "refresh_token");
        assert_eq!(fields["refresh_token"], "refresh+/&");
        assert!(!fields.contains_key("code"));
        Response::builder()
            .header("content-type", "application/json")
            .body(Body::from(OAUTH))
            .unwrap()
    }

    #[tokio::test]
    async fn generated_sdk_keeps_the_exchange_and_oauth_encoding_intact() {
        let files = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(files.path());
        let client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
            .with(ExchangeRecorder {
                store: store.clone(),
            })
            .build();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route("/api/v3/activities/:id", get(activity))
            .route("/oauth/token", post(oauth));
        let server = tokio::spawn(async {
            axum::serve(listener, app).await.unwrap();
        });
        let config = Configuration {
            client,
            base_path: format!("{origin}/api/v3"),
            oauth_access_token: Some("fixture-access".into()),
            ..Configuration::default()
        };
        let (decoded, original) = capture(activities_api::get_activity_by_id(
            &config,
            20424619693,
            Some(false),
        ))
        .await;
        assert_eq!(decoded.unwrap().id, Some(20424619693));
        let original = original.unwrap();
        assert_eq!(original.bytes.as_ref(), ACTIVITY);
        assert_eq!(original.headers["x-vendor-request"], "original-response");
        let oauth_config = Configuration {
            base_path: origin,
            oauth_access_token: None,
            ..config
        };
        let (decoded, original) = capture(strava_api::oauth_token(
            &oauth_config,
            "fixture-client",
            "fixture-secret",
            "refresh_token",
            None,
            Some("refresh+/&"),
        ))
        .await;
        assert_eq!(decoded.unwrap()["access_token"], "vendor-secret");
        assert_eq!(original.unwrap().bytes.as_ref(), OAUTH);
        assert_saved_exchanges(files.path(), &store).await;
        server.abort();
    }

    async fn assert_saved_exchanges(root: &std::path::Path, store: &ObjectStore) {
        let mut exchanges = tokio::fs::read_dir(root.join("strava/exchanges"))
            .await
            .unwrap();
        let mut count = 0;
        while let Some(exchange) = exchanges.next_entry().await.unwrap() {
            let path = exchange.path();
            let result = read_encrypted_result(&path).await;
            let body = STANDARD
                .decode(result["body_base64"].as_str().unwrap())
                .unwrap();
            assert!(body == ACTIVITY || body == OAUTH);
            if body == ACTIVITY {
                let saved = store
                    .response(&format!(
                        "strava/exchanges/{}/response",
                        exchange.file_name().to_str().unwrap()
                    ))
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(store.read(&saved.body).await.unwrap(), ACTIVITY);
            } else {
                let saved = store
                    .response(&format!(
                        "strava/exchanges/{}/response",
                        exchange.file_name().to_str().unwrap()
                    ))
                    .await
                    .unwrap()
                    .unwrap();
                assert!(saved.encrypted);
                assert!(!String::from_utf8(store.read(&saved.body).await.unwrap())
                    .unwrap()
                    .contains("vendor-secret"));
                assert_eq!(store.response_bytes(&saved).await.unwrap(), OAUTH);
            }
            count += 1;
        }
        assert_eq!(count, 2);
    }

    async fn read_encrypted_result(path: &std::path::Path) -> serde_json::Value {
        let mut results = tokio::fs::read_dir(path.join("result")).await.unwrap();
        let bytes = tokio::fs::read(results.next_entry().await.unwrap().unwrap().path())
            .await
            .unwrap();
        let encrypted = String::from_utf8(bytes).unwrap();
        assert!(encrypted.starts_with("enc:v1:"));
        assert!(!encrypted.contains("vendor-secret"));
        serde_json::from_str(&credentials::open(&encrypted).unwrap()).unwrap()
    }
}
