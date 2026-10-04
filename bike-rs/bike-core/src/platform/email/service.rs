use lettre::message::header::ContentType;
use lettre::message::{header, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use std::error::Error;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from_email: String,
    pub from_name: String,
}

#[derive(Debug, Clone)]
struct ResendIdempotencyKey(String);

impl header::Header for ResendIdempotencyKey {
    fn name() -> header::HeaderName {
        header::HeaderName::new_from_ascii_str("Resend-Idempotency-Key")
    }

    fn parse(_: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        unimplemented!("Parsing not needed for outgoing headers")
    }

    fn display(&self) -> header::HeaderValue {
        header::HeaderValue::new(
            header::HeaderName::new_from_ascii_str("Resend-Idempotency-Key"),
            self.0.clone(),
        )
    }
}

pub struct EmailService {
    transport: SmtpTransport,
    from_email: String,
    from_name: String,
}

impl EmailService {
    pub fn new(config: &SmtpConfig) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let transport =
            if let (Some(username), Some(password)) = (&config.username, &config.password) {
                let credentials = Credentials::new(username.clone(), password.clone());
                SmtpTransport::starttls_relay(&config.host)?
                    .port(config.port)
                    .credentials(credentials)
                    .build()
            } else {
                SmtpTransport::builder_dangerous(&config.host)
                    .port(config.port)
                    .build()
            };

        Ok(Self {
            transport,
            from_email: config.from_email.clone(),
            from_name: config.from_name.clone(),
        })
    }

    pub async fn send(
        &self,
        to: &str,
        subject: &str,
        text_body: String,
        html_body: String,
        idempotency_key: Option<String>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let span = tracing::info_span!(
            "email.send",
            email.transport = "smtp",
            email.has_idempotency_key = idempotency_key.is_some(),
            email.status = tracing::field::Empty,
            error = tracing::field::Empty,
        );
        let _guard = span.enter();

        let email = self.build_message(to, subject, text_body, html_body, idempotency_key)?;
        match self.transport.send(&email) {
            Ok(_) => {
                span.record("email.status", "sent");
                info!(to, "Email sent successfully");
                Ok(())
            }
            Err(error) => {
                span.record("email.status", "failed");
                span.record("error", error.to_string().as_str());
                warn!(to, %error, "Failed to send email");
                Err(Box::new(error))
            }
        }
    }

    fn build_message(
        &self,
        to: &str,
        subject: &str,
        text_body: String,
        html_body: String,
        idempotency_key: Option<String>,
    ) -> Result<Message, Box<dyn Error + Send + Sync>> {
        let from_mailbox = Mailbox::new(Some(self.from_name.clone()), self.from_email.parse()?);
        let to_mailbox: Mailbox = to.parse()?;

        let mut email_builder = Message::builder()
            .from(from_mailbox)
            .to(to_mailbox)
            .subject(subject);

        if let Some(key) = idempotency_key {
            email_builder = email_builder.header(ResendIdempotencyKey(key));
        }

        Ok(email_builder.multipart(
            MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_PLAIN)
                        .body(text_body),
                )
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body(html_body),
                ),
        )?)
    }
}
