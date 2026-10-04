//! Bike account identity, local auto-login, SSO, and browser sessions.
pub mod controllers;
pub mod cookies;
pub mod entities;
pub mod error;
pub mod extractors;
pub mod openapi;
pub mod services;
pub mod tokens;
pub use controllers::admin::routes as admin_routes;
pub use controllers::auth::{session_routes, AuthRouteStorage};
pub use controllers::oauth::routes as oauth_routes;
pub use error::AuthError;
pub use extractors::{AdminUserContext, AuthStorage, UserContext};
pub use services::{
    oauth::OAuthService, oauth_provider_service::OAuthProviderService,
    provider_settings::PROVIDER_DEV, SessionService,
};

#[cfg(test)]
mod tests;
