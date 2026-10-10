use super::types::{MapImageError, RenderRequest, Theme, Variant};
use std::time::Duration;
use tonic::{
    metadata::MetadataValue,
    transport::{Channel, Endpoint},
    Request,
};
use tracing::Instrument;

pub mod proto {
    tonic::include_proto!("bike.maps.v1");
}

pub(super) struct SnapshotClient {
    client: proto::map_service_client::MapServiceClient<Channel>,
    authorization: Option<MetadataValue<tonic::metadata::Ascii>>,
    address: String,
    port: u16,
}

impl SnapshotClient {
    pub fn new(address: String, token: String) -> Result<Self, MapImageError> {
        let endpoint = Endpoint::from_shared(address)?
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(60));
        let authorization = if token.is_empty() {
            None
        } else {
            Some(
                format!("Bearer {token}")
                    .parse()
                    .map_err(|_| MapImageError::Credentials)?,
            )
        };
        Ok(Self {
            address: endpoint.uri().host().unwrap_or_default().to_owned(),
            port: endpoint.uri().port_u16().unwrap_or(80),
            client: proto::map_service_client::MapServiceClient::new(endpoint.connect_lazy())
                .max_encoding_message_size(10_000_000)
                .max_decoding_message_size(20_000_000),
            authorization,
        })
    }

    pub async fn snapshot(&self, input: &RenderRequest) -> Result<Vec<u8>, MapImageError> {
        let span = tracing::info_span!("bike.maps.snapshot",
            otel.kind = "client", rpc.system = "grpc", rpc.service = "bike.maps.v1.MapService",
            rpc.method = "Render", server.address = %self.address, server.port = self.port,
            rpc.grpc.status_code = tracing::field::Empty, otel.status_code = tracing::field::Empty,
            error.message = tracing::field::Empty);
        self.call(input).instrument(span).await
    }

    async fn call(&self, input: &RenderRequest) -> Result<Vec<u8>, MapImageError> {
        let mut request = Request::new(input.to_proto());
        request.set_timeout(Duration::from_secs(60));
        if let Some(value) = &self.authorization {
            request
                .metadata_mut()
                .insert("authorization", value.clone());
        }
        for (key, value) in crate::observability::inject_current_trace_context().unwrap_or_default()
        {
            if let (Ok(key), Ok(value)) = (
                key.parse::<tonic::metadata::MetadataKey<tonic::metadata::Ascii>>(),
                value.parse(),
            ) {
                request.metadata_mut().insert(key, value);
            }
        }
        let response = self.client.clone().render(request).await.map_err(|error| {
            let span = tracing::Span::current();
            span.record("rpc.grpc.status_code", error.code() as i32);
            span.record("otel.status_code", "ERROR");
            span.record("error.message", error.to_string());
            MapImageError::Snapshot(Box::new(error))
        })?;
        tracing::Span::current().record("rpc.grpc.status_code", 0);
        let png = response.into_inner().png;
        if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
            tracing::Span::current().record("otel.status_code", "ERROR");
            tracing::Span::current().record("error.message", "Invalid PNG response");
            return Err(MapImageError::InvalidPNG);
        }
        Ok(png)
    }
}

impl RenderRequest {
    fn to_proto(&self) -> proto::RenderMapRequest {
        proto::RenderMapRequest {
            theme: match self.theme {
                Theme::Light => "light",
                Theme::Dark => "dark",
            }
            .into(),
            variant: match self.variant {
                Variant::Full => "full",
                Variant::Thumbnail => "thumbnail",
            }
            .into(),
            dpr: self.dpr.into(),
            points: self
                .points
                .iter()
                .map(|point| proto::RoutePoint {
                    latitude: point.latitude,
                    longitude: point.longitude,
                })
                .collect(),
        }
    }
}
