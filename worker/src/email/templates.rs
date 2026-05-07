use kaleido::glass::email::TemplateRegistry;
use serde::Serialize;
use std::error::Error;

pub struct EmailTemplate {
    templates: TemplateRegistry,
}

impl EmailTemplate {
    pub fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let mut templates = TemplateRegistry::new();
        templates.register_template(
            "notification_text",
            include_str!("templates/notification.txt"),
        )?;
        templates.register_template(
            "notification_html",
            include_str!("templates/notification.html"),
        )?;

        Ok(Self { templates })
    }

    pub fn render<T: Serialize>(
        &self,
        template_name: &str,
        data: &T,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        self.templates.render(template_name, data)
    }
}
