use super::*;
use axum::body::to_bytes;

#[tokio::test]
async fn unit_happy_platform_errors_preserve_status_and_public_message() {
    let cases = [
        (
            PlatformError::unauthorized("Sign in"),
            StatusCode::UNAUTHORIZED,
        ),
        (
            PlatformError::forbidden("Admin only"),
            StatusCode::FORBIDDEN,
        ),
        (
            PlatformError::validation("Choose a sport"),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            PlatformError::conflict("Import running"),
            StatusCode::CONFLICT,
        ),
        (
            PlatformError::entity_not_found("Ride not found"),
            StatusCode::NOT_FOUND,
        ),
        (
            PlatformError::internal_error("Try again"),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    ];
    for (error, expected_status) in cases {
        let message = error.to_string();
        let response = error.into_response();
        assert_eq!(response.status(), expected_status);
        let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body, serde_json::json!({"error": message}));
    }
}
