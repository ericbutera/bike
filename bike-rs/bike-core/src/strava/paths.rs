//! The retained Strava object layout. Keep these keys stable across code moves.
use super::objects::checksum;
use crate::entities::strava_connections;

pub const ROOT: &str = "strava";

pub(crate) fn authorization(code: &str) -> String {
    format!(
        "{ROOT}/credentials/authorization/{}",
        checksum(code.as_bytes())
    )
}

pub(crate) fn refresh(connection: &strava_connections::Model, token: &str) -> String {
    let identity = format!("{token}:{}", connection.expires_at.timestamp());
    format!(
        "{ROOT}/{}/credentials/{}/refresh/{}",
        connection.athlete_id,
        connection.created_at.timestamp_micros(),
        checksum(identity.as_bytes())
    )
}

pub(crate) fn exchange(archive: Option<&str>, id: &str) -> String {
    match archive {
        Some(archive) => format!("{archive}/exchanges/{id}"),
        None => format!("{ROOT}/exchanges/{id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_and_exchange_keys_are_stable() {
        assert_eq!(exchange(None, "request"), "strava/exchanges/request");
        assert_eq!(
            exchange(Some("retained"), "request"),
            "retained/exchanges/request"
        );
        assert_eq!(
            authorization("code"),
            format!("strava/credentials/authorization/{}", checksum(b"code"))
        );
    }
}
