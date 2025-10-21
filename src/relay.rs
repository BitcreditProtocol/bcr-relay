use anyhow::Result;
use clap::Parser;
use deadpool_postgres::Pool;
use nostr::types::Url;
use nostr_postgres_db::*;
use nostr_relay_builder::{
    LocalRelay, RelayBuilder,
    builder::{RelayBuilderNip42, RelayBuilderNip42Mode},
};

pub async fn init(config: &RelayConfig, pool: Pool) -> Result<LocalRelay> {
    Ok(LocalRelay::new(builder(config, pool).await?).await?)
}

async fn builder(_config: &RelayConfig, pool: Pool) -> Result<RelayBuilder> {
    let dba = database(pool).await?;
    Ok(RelayBuilder::default().nip42(auth_mode()).database(dba))
}

fn auth_mode() -> RelayBuilderNip42 {
    RelayBuilderNip42 {
        // read and write requires client auth
        mode: RelayBuilderNip42Mode::Both,
    }
}

async fn database(pool: Pool) -> Result<NostrPostgres> {
    Ok(NostrPostgres::from_pool(pool).await?)
}

#[derive(Debug, Clone, Parser)]
pub struct RelayConfig {
    #[arg(default_value_t = String::from("localhost:8080"), long, env = "LISTEN_ADDRESS")]
    pub listen_address: String,
    #[arg(default_value_t = Url::parse("http://localhost:8080").unwrap(), long, env = "HOST_URL")]
    pub host_url: Url,

    #[arg(default_value_t = String::from("postgres"), long, env = "DB_USER")]
    pub db_user: String,
    #[arg(default_value_t = String::from("password"), long, env = "DB_PASSWORD")]
    pub db_password: String,
    #[arg(default_value_t = String::from(""), long, env = "DB_NAME")]
    pub db_name: String,
    #[arg(default_value_t = String::from("localhost"), long, env = "DB_HOST")]
    pub db_host: String,
    #[arg(default_value_t = String::from(""), long, env = "EMAIL_FROM_ADDRESS")]
    pub email_from_address: String,
    #[arg(default_value_t = String::from(""), long, env = "EMAIL_API_KEY")]
    pub email_api_key: String,
    #[arg(default_value_t = String::from(""), long, env = "EMAIL_API_SECRET_KEY")]
    pub email_api_secret_key: String,
    #[arg(default_value_t = Url::parse("https://api.mailjet.com").unwrap(), long, env = "EMAIL_URL")]
    pub email_url: Url,
}

impl RelayConfig {
    pub fn db_connection_string(&self) -> String {
        let db_name = if self.db_name.is_empty() {
            "".to_string()
        } else {
            format!("/{}", self.db_name)
        };
        format!(
            "postgres://{}:{}@{}?host={}",
            self.db_user, self.db_password, db_name, self.db_host
        )
    }
}
