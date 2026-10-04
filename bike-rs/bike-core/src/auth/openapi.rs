pub mod paths {
    pub use crate::auth::controllers::admin::{
        __path_disable_user, __path_get_user, __path_list_users, __path_update_user, disable_user,
        get_user, list_users, update_user,
    };
    pub use crate::auth::controllers::auth::{
        __path_current, __path_logout, __path_refresh, current, logout, refresh,
    };
    pub use crate::auth::controllers::oauth::{
        __path_oauth_authorize, __path_oauth_callback, __path_oauth_providers, oauth_authorize,
        oauth_callback, oauth_providers,
    };
}
pub mod schemas {
    pub use crate::auth::controllers::admin::{
        AdminUserResponse, AdminUsersListResponse, DisableUserRequest, UpdateUserRequest,
    };
    pub use crate::auth::controllers::oauth::OAuthProvidersResponse;
    pub use crate::auth::services::oauth_provider_service::OAuthProviderMetadata;
    pub use crate::auth::services::{MessageResponse, TokenResponse, UserResponse};
}
