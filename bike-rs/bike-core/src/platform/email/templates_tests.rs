use super::*;

#[test]
fn unit_happy_registered_template_renders_rider_data() {
    let mut templates = TemplateRegistry::default();
    templates
        .register_template("welcome", "Hello {{name}}, your ride is ready.")
        .unwrap();
    let rendered = templates
        .render("welcome", &serde_json::json!({"name": "Rider"}))
        .unwrap();
    assert_eq!(rendered, "Hello Rider, your ride is ready.");
}
