use async_trait::async_trait;

pub mod mailjet;

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

pub fn build_email_confirmation_message(
    host_url: &url::Url,
    from: &str,
    to: &str,
    token: &str,
) -> EmailMessage {
    // build email confirmation link
    let mut link = host_url
        .join("/notifications/confirm_email")
        .expect("email confirmation mail");
    link.set_query(Some(&format!("token={token}")));

    // build template
    let body = format!(
        "<html><head></head><body><a href=\"{link}\">Click here to confirm</a><br /><br />This link is valid for 1 day.</body></html>"
    );

    EmailMessage {
        from: from.to_owned(),
        to: to.to_owned(),
        subject: "Confirm your E-Mail".to_owned(),
        body: body.to_owned(),
    }
}
