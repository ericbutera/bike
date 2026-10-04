//! Helper functions for issuing and clearing refresh-token cookies.

/// Name of the HttpOnly cookie used for refresh tokens across the starter kit.
pub const REFRESH_COOKIE_NAME: &str = "refresh_token";
pub const OAUTH_STATE_COOKIE_NAME: &str = "oauth_state";

pub fn oauth_state_cookie_value(state: &str, frontend_url: &str) -> String {
    oauth_cookie(state, frontend_url, 600)
}

pub fn clear_oauth_state_cookie_value(frontend_url: &str) -> String {
    oauth_cookie("", frontend_url, 0)
}

fn oauth_cookie(state: &str, frontend_url: &str, max_age: u32) -> String {
    let secure = if frontend_url.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    format!("{OAUTH_STATE_COOKIE_NAME}={state}; HttpOnly; Path=/api/oauth; SameSite=Lax; Max-Age={max_age}{secure}")
}

pub fn validate_oauth_state(
    state: Option<&str>,
    headers: &axum::http::HeaderMap,
) -> Result<String, super::AuthError> {
    let state = state
        .filter(|state| !state.is_empty())
        .ok_or_else(|| super::AuthError::bad_request("OAuth callback state is required"))?;
    let cookie = headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .find_map(|cookie| cookie.trim().strip_prefix("oauth_state="));
    if cookie.map(super::entities::oauth_states::Model::hash)
        != Some(super::entities::oauth_states::Model::hash(state))
    {
        return Err(super::AuthError::bad_request(
            "OAuth state does not match this browser",
        ));
    }
    Ok(state.to_owned())
}

/// Build a `Set-Cookie` value for the HttpOnly refresh cookie.
///
/// # Arguments
/// * `token` - The refresh token value
/// * `frontend_url` - The frontend URL to determine if connection is secure
///
/// Number of seconds in 7 days — matches the refresh token TTL in the database.
const REFRESH_COOKIE_MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60;

pub fn refresh_cookie_value(token: &str, frontend_url: &str) -> String {
    let secure = frontend_url.starts_with("https://");
    let mut cookie_val = format!(
        "{}={}; HttpOnly; Path=/; SameSite=Strict; Max-Age={};",
        REFRESH_COOKIE_NAME, token, REFRESH_COOKIE_MAX_AGE_SECS
    );
    if secure {
        cookie_val.push_str(" Secure;");
    }
    cookie_val
}

/// Build a `Set-Cookie` value that clears the refresh cookie immediately.
///
/// # Arguments
/// * `frontend_url` - The frontend URL to determine if connection is secure
pub fn clear_refresh_cookie_value(frontend_url: &str) -> String {
    let secure = frontend_url.starts_with("https://");
    let mut cookie_val = format!(
        "{}=; HttpOnly; Path=/; SameSite=Strict; Max-Age=0;",
        REFRESH_COOKIE_NAME
    );
    if secure {
        cookie_val.push_str(" Secure;");
    }
    cookie_val
}
