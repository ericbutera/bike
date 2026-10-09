use super::*;

#[test]
fn unit_happy_notifications_render_text_and_html_from_the_same_payload() {
    let templates = EmailTemplate::new().unwrap();
    let payload = serde_json::json!({
        "app_name": "Bike",
        "subject": "Ride ready",
        "message": "Your ride has been processed."
    });
    let text = templates.render("notification_text", &payload).unwrap();
    let html = templates.render("notification_html", &payload).unwrap();

    assert!(text.contains("Bike notification"));
    assert!(text.contains("Ride ready"));
    assert!(text.contains("Your ride has been processed."));
    assert!(html.contains("<h1>Bike notification</h1>"));
    assert!(html.contains("<strong>Ride ready</strong>"));
    assert!(html.contains("<p>Your ride has been processed.</p>"));
}
