//! Transactional email sender for PH Press — SERVER-ONLY. Currently Amazon SES
//! via the SESv2 `SendEmail` API, using the standard AWS credential chain: on EC2
//! that resolves to the instance role over IMDS, so no keys live in the container
//! (the same pattern the Bedrock client in `ph-ai` uses). Selected by
//! `PH_EMAIL_BACKEND`; when unset the whole feature is off and callers fall back
//! to logging the reset link.

#[derive(Debug)]
pub enum EmailError {
    /// The email request was rejected (unverified sender, sandbox, throttling…).
    Send(String),
    /// A value could not be built into the SDK request.
    Build(String),
}

impl std::fmt::Display for EmailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmailError::Send(m) => write!(f, "email send failed: {m}"),
            EmailError::Build(m) => write!(f, "email build failed: {m}"),
        }
    }
}
impl std::error::Error for EmailError {}

/// The configured delivery backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailBackend {
    Ses,
}

/// Resolved sender configuration. Obtained from [`EmailConfig::from_env`], which
/// returns `None` when delivery isn't configured.
#[derive(Debug, Clone)]
pub struct EmailConfig {
    pub backend: EmailBackend,
    /// Verified sender, e.g. `"Predator Hunters <no-reply@predatorhunters.co.uk>"`.
    pub from: String,
    /// AWS region for the SES endpoint.
    pub region: String,
}

impl EmailConfig {
    /// Build from the environment, or `None` when delivery is off:
    ///   `PH_EMAIL_BACKEND`  `"ses"` (anything else / unset → `None`)
    ///   `PH_EMAIL_FROM`     verified SES sender address (required)
    ///   `PH_EMAIL_REGION`   AWS region (falls back to `AWS_REGION`, then `eu-west-2`)
    pub fn from_env() -> Option<Self> {
        let backend = match std::env::var("PH_EMAIL_BACKEND").ok()?.trim().to_lowercase().as_str() {
            "ses" => EmailBackend::Ses,
            _ => return None,
        };
        let from = std::env::var("PH_EMAIL_FROM")
            .ok()
            .filter(|s| !s.trim().is_empty())?;
        let region = std::env::var("PH_EMAIL_REGION")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| std::env::var("AWS_REGION").ok().filter(|s| !s.is_empty()))
            .unwrap_or_else(|| "eu-west-2".to_string());
        Some(Self { backend, from, region })
    }
}

/// A single message to send. Both a plain-text and an HTML part are supplied; the
/// recipient's client picks the richest it can render.
pub struct Email<'a> {
    pub to: &'a str,
    pub subject: &'a str,
    pub text: &'a str,
    pub html: &'a str,
}

/// Send `msg` via the configured backend. Returns the provider message id.
pub async fn send(cfg: &EmailConfig, msg: &Email<'_>) -> Result<String, EmailError> {
    match cfg.backend {
        EmailBackend::Ses => send_ses(cfg, msg).await,
    }
}

async fn send_ses(cfg: &EmailConfig, msg: &Email<'_>) -> Result<String, EmailError> {
    use aws_sdk_sesv2::types::{Body, Content, Destination, EmailContent, Message};

    let aws_cfg = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new(cfg.region.clone()))
        .load()
        .await;
    let client = aws_sdk_sesv2::Client::new(&aws_cfg);

    let content = |data: &str| -> Result<Content, EmailError> {
        Content::builder()
            .data(data)
            .charset("UTF-8")
            .build()
            .map_err(|e| EmailError::Build(format!("{e:?}")))
    };
    let body = Body::builder()
        .text(content(msg.text)?)
        .html(content(msg.html)?)
        .build();
    let message = Message::builder()
        .subject(content(msg.subject)?)
        .body(body)
        .build();
    let destination = Destination::builder().to_addresses(msg.to).build();

    let out = client
        .send_email()
        .from_email_address(&cfg.from)
        .destination(destination)
        .content(EmailContent::builder().simple(message).build())
        .send()
        .await
        .map_err(|e| EmailError::Send(format!("{e:?}")))?;
    Ok(out.message_id().unwrap_or_default().to_string())
}
