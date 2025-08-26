use async_trait::async_trait;
use serde::Serialize;
use tinytemplate::TinyTemplate;

use crate::util::get_logo_link;

pub mod mailjet;
mod template;

#[async_trait]
pub trait EmailService: Send + Sync {
    async fn send(&self, msg: EmailMessage) -> Result<(), anyhow::Error>;
}

/// A simple email message. We can add more features (like html, multi recipient, etc.) later.
#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body: String,
}

#[derive(Serialize)]
struct EmailConfirmationContext {
    pub logo_link: String,
    pub link: String,
}

pub fn build_email_confirmation_message(
    host_url: &url::Url,
    from: &str,
    to: &str,
    token: &str,
) -> Result<EmailMessage, anyhow::Error> {
    let mut tt = TinyTemplate::new();
    tt.add_template("mail", template::MAIL_CONFIRMATION_TEMPLATE)?;

    // build email confirmation link
    let mut link = host_url
        .join("/notifications/confirm_email")
        .expect("email confirmation mail");
    link.set_query(Some(&format!("token={token}")));

    let context = EmailConfirmationContext {
        logo_link: get_logo_link(host_url),
        link: link.to_string(),
    };

    let rendered = tt.render("mail", &context)?;

    Ok(EmailMessage {
        from: from.to_owned(),
        to: to.to_owned(),
        subject: "Please confirm your E-Mail".to_owned(),
        body: rendered,
    })
}
