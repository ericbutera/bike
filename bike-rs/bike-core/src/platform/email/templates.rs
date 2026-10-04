use handlebars::Handlebars;
use serde::Serialize;
use std::error::Error;

pub struct TemplateRegistry {
    handlebars: Handlebars<'static>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        Self {
            handlebars: Handlebars::new(),
        }
    }

    pub fn register_template(
        &mut self,
        name: &str,
        template: &'static str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.handlebars.register_template_string(name, template)?;
        Ok(())
    }

    pub fn render<T: Serialize>(
        &self,
        template_name: &str,
        data: &T,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        Ok(self.handlebars.render(template_name, data)?)
    }
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}
