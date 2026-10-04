use super::{provider_settings::ProviderConfig, OAuthUserInfo};
use crate::auth::error::AuthError;
use openidconnect::core::{CoreClient, CoreProviderMetadata};
use openidconnect::{
    AuthType, AuthorizationCode, ClientId, ClientSecret, IssuerUrl, Nonce, RedirectUrl,
    TokenResponse,
};

pub async fn exchange(
    cfg: &ProviderConfig,
    code: String,
    nonce: String,
) -> Result<OAuthUserInfo, AuthError> {
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| AuthError::internal_error("Failed to build OIDC client"))?;
    let issuer = IssuerUrl::new(cfg.issuer_url.clone().unwrap_or_default())
        .map_err(|_| AuthError::internal_error("Invalid OIDC issuer URL"))?;
    let metadata = CoreProviderMetadata::discover_async(issuer, &http)
        .await
        .map_err(|_| AuthError::internal_error("OIDC discovery failed"))?;
    let client = CoreClient::from_provider_metadata(
        metadata,
        ClientId::new(cfg.client_id.clone()),
        Some(ClientSecret::new(cfg.client_secret.clone())),
    )
    .set_redirect_uri(
        RedirectUrl::new(cfg.redirect_url.clone())
            .map_err(|_| AuthError::internal_error("Invalid redirect URL"))?,
    )
    .set_auth_type(AuthType::RequestBody);
    let tokens = client
        .exchange_code(AuthorizationCode::new(code))
        .map_err(|_| AuthError::internal_error("OIDC token endpoint is missing"))?
        .request_async(&http)
        .await
        .map_err(|_| AuthError::bad_request("Provider rejected the authorization code"))?;
    let id_token = tokens
        .id_token()
        .ok_or_else(|| AuthError::bad_request("OAuth provider did not return an ID token"))?;
    let verifier = client.id_token_verifier();
    let claims = id_token
        .claims(&verifier, &Nonce::new(nonce))
        .map_err(|_| AuthError::bad_request("OIDC ID token validation failed"))?;
    if claims.email_verified() == Some(false) {
        return Err(AuthError::validation("Email not verified by provider"));
    }
    Ok(OAuthUserInfo {
        subject: claims.subject().as_str().to_owned(),
        email: claims
            .email()
            .ok_or_else(|| AuthError::bad_request("Provider identity did not include email"))?
            .as_str()
            .to_owned(),
        name: claims
            .name()
            .and_then(|name| name.get(None))
            .map(|name| name.as_str().to_owned()),
        preferred_username: claims
            .preferred_username()
            .map(|name| name.as_str().to_owned()),
        verified_email: claims.email_verified(),
    })
}
