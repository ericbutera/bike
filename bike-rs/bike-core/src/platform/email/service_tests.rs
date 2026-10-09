use super::*;

#[test]
fn unit_happy_email_builds_alternative_bodies_and_delivery_identity() {
    let service = EmailService::new(&SmtpConfig {
        host: "localhost".into(),
        port: 2525,
        username: None,
        password: None,
        from_email: "bike@example.com".into(),
        from_name: "Bike".into(),
    })
    .unwrap();
    let message = service
        .build_message(
            "rider@example.com",
            "Ride ready",
            "Your ride is ready.".into(),
            "<p>Your ride is ready.</p>".into(),
            Some("activity-7".into()),
        )
        .unwrap();
    let formatted = String::from_utf8(message.formatted()).unwrap();

    assert!(formatted.contains("From: Bike <bike@example.com>"));
    assert!(formatted.contains("To: rider@example.com"));
    assert!(formatted.contains("Subject: Ride ready"));
    assert!(formatted.contains("Content-Type: multipart/alternative"));
    assert!(formatted.contains("Content-Type: text/plain"));
    assert!(formatted.contains("Content-Type: text/html"));
    assert!(formatted.contains("Resend-Idempotency-Key: activity-7"));
}
