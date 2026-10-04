use utoipa::openapi::server::Server;

// Re-exported path/schema helpers for glass-hosted features (feature flags, etc.)
pub mod paths {
    pub use crate::platform::feature_flags::admin_controller::{list_flags, update_flag};
    pub use crate::platform::feature_flags::controller::public_flags;
    pub use crate::platform::metrics_controller::get_metrics;

    // expose utoipa marker types for the paths
    pub use crate::platform::feature_flags::admin_controller::{
        __path_list_flags, __path_update_flag,
    };
    pub use crate::platform::feature_flags::controller::__path_public_flags;
    pub use crate::platform::metrics_controller::__path_get_metrics;
}

pub mod schemas {
    pub use crate::platform::data::pagination::PaginatedResponse;
    pub use crate::platform::feature_flags::admin_controller::{
        FeatureFlagResponse, UpdateFlagRequest,
    };
    pub use crate::platform::feature_flags::controller::PublicFlagResponse;
    pub use crate::platform::system_metrics::SystemMetrics;
}

pub mod tags {
    pub const TAGS: &[(&str, &str)] = &[
        ("flags", "Feature flags"),
        ("admin", "Admin-only endpoints"),
    ];
}

/// Shared OpenAPI modifier used by services to add security scheme
/// and ensure the `/api` server entry is present.
#[derive(Clone, Copy)]
pub struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::HttpBuilder::new()
                        .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            )
        }

        match openapi.servers.as_mut() {
            Some(servers) => {
                let has_api = servers.iter().any(|s| s.url == "/api");
                if !has_api {
                    servers.insert(0, Server::new("/api"));
                }
            }
            None => {
                openapi.servers = Some(vec![Server::new("/api")]);
            }
        }
    }
}
