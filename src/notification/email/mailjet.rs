use crate::notification::email::{EmailMessage, EmailService};
use anyhow::anyhow;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::error;

#[derive(Debug, Clone)]
pub struct MailjetConfig {
    pub api_key: String,
    pub api_secret_key: String,
    pub url: url::Url,
}

pub struct MailjetService {
    config: MailjetConfig,
    client: reqwest::Client,
}

impl MailjetService {
    pub fn new(config: &MailjetConfig) -> Self {
        let client = reqwest::Client::new();
        Self {
            config: config.to_owned(),
            client,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct MailjetReq {
    #[serde(rename = "Messages")]
    pub messages: Vec<MailjetMessage>,
}

#[derive(Debug, Clone, Deserialize)]
struct MailjetResp {
    #[serde(rename = "Messages")]
    pub messages: Vec<MailjetRespMessage>,
}

#[derive(Debug, Clone, Serialize)]
struct MailjetMessage {
    #[serde(rename = "From")]
    pub from: MailjetFrom,
    #[serde(rename = "To")]
    pub to: Vec<MailjetTo>,
    #[serde(rename = "Subject")]
    pub subject: String,
    #[serde(rename = "HTMLPart")]
    pub html_part: String,
}

impl From<EmailMessage> for MailjetMessage {
    fn from(value: EmailMessage) -> Self {
        Self {
            from: MailjetFrom { email: value.from },
            to: vec![MailjetTo { email: value.to }],
            subject: value.subject,
            html_part: value.body,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct MailjetFrom {
    #[serde(rename = "Email")]
    pub email: String,
}

#[derive(Debug, Clone, Serialize)]
struct MailjetTo {
    #[serde(rename = "Email")]
    pub email: String,
}

#[derive(Debug, Clone, Deserialize)]
struct MailjetRespMessage {
    #[serde(rename = "Status")]
    pub status: String,
}

#[async_trait]
impl EmailService for MailjetService {
    async fn send(&self, msg: super::EmailMessage) -> Result<(), anyhow::Error> {
        let mailjet_msg = MailjetReq {
            messages: vec![MailjetMessage::from(msg)],
        };

        let url = self.config.url.join("/v3.1/send").expect("mailjet path");
        let request = self.client.post(url).json(&mailjet_msg).basic_auth(
            self.config.api_key.clone(),
            Some(self.config.api_secret_key.clone()),
        );
        let res = request.send().await.map_err(|e| {
            error!("Failed to send email: {e}");
            anyhow!("Failed to send email")
        })?;

        let resp: MailjetResp = res.json().await.map_err(|e| {
            error!("Failed to parse email response: {e}");
            anyhow!("Failed to parse email response")
        })?;

        match resp.messages.first() {
            Some(msg) => {
                if msg.status != "success" {
                    error!("Invalid email sending response: {}", &msg.status);
                    Err(anyhow!("Invalid email sending response: {}", &msg.status))
                } else {
                    Ok(())
                }
            }
            None => {
                error!("Invalid email response - got no status");
                Err(anyhow!("Invalid email response - got no status"))
            }
        }
    }
}
