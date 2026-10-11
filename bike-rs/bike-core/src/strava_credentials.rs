//! Credential values and original OAuth responses are encrypted separately.
use crate::{config::Config, workflow_error::WorkflowError};
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm,
};
use base64::{engine::general_purpose::STANDARD, Engine};

fn cipher() -> Result<Aes256Gcm, WorkflowError> {
    let key = match std::env::var("STRAVA_TOKEN_ENCRYPTION_KEY") {
        Ok(key) => STANDARD
            .decode(key)
            .map_err(|_| WorkflowError::internal("Invalid Strava credential encryption key"))?,
        Err(_) if Config::get().environment != "production" => {
            use sha2::Digest;
            sha2::Sha256::digest(format!(
                "bike:strava:credentials:{}",
                Config::get().jwt_secret
            ))
            .to_vec()
        }
        Err(_) => {
            return Err(WorkflowError::internal(
                "STRAVA_TOKEN_ENCRYPTION_KEY is required",
            ))
        }
    };
    Aes256Gcm::new_from_slice(&key)
        .map_err(|_| WorkflowError::internal("Strava credential key must contain 32 bytes"))
}

pub fn seal(bytes: &[u8]) -> Result<String, WorkflowError> {
    let cipher = cipher()?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let encrypted = cipher
        .encrypt(&nonce, bytes)
        .map_err(|_| WorkflowError::internal("Failed to encrypt Strava credentials"))?;
    Ok(format!(
        "enc:v1:{}",
        STANDARD.encode([nonce.as_slice(), &encrypted].concat())
    ))
}

pub fn open(value: &str) -> Result<String, WorkflowError> {
    // Existing mirror credentials are readable during migration; all new writes
    // are sealed. Cutover verification rejects remaining plaintext values.
    let Some(encoded) = value.strip_prefix("enc:v1:") else {
        return Ok(value.to_owned());
    };
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| WorkflowError::internal("Invalid encrypted Strava credential"))?;
    let nonce: [u8; 12] = bytes
        .get(..12)
        .ok_or_else(|| WorkflowError::internal("Truncated Strava credential"))?
        .try_into()
        .map_err(|_| WorkflowError::internal("Invalid Strava credential nonce"))?;
    let decrypted = cipher()?
        .decrypt(&nonce.into(), &bytes[12..])
        .map_err(|_| WorkflowError::internal("Failed to decrypt Strava credentials"))?;
    String::from_utf8(decrypted)
        .map_err(|_| WorkflowError::internal("Invalid Strava credential encoding"))
}
