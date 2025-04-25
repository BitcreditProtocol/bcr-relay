use anyhow::Result;
use clap::Parser;
use nostr_relay_builder::{
    LocalRelay, RelayBuilder,
    builder::{RelayBuilderNip42, RelayBuilderNip42Mode},
};
use nostr_sqldb::*;
use tracing::info;

pub async fn init(config: &RelayConfig) -> Result<LocalRelay> {
    Ok(LocalRelay::new(builder(config).await?).await?)
}

async fn builder(config: &RelayConfig) -> Result<RelayBuilder> {
    let dba = database(&config.db_connection_string).await?;
    Ok(RelayBuilder::default().nip42(auth_mode()).database(dba))
}

fn auth_mode() -> RelayBuilderNip42 {
    RelayBuilderNip42 {
        // read and write requires client auth
        mode: RelayBuilderNip42Mode::Both,
    }
}

async fn database(db_url: &str) -> Result<NostrPostgres> {
    info!("Starting database migrations on {}", db_url);
    run_migrations(db_url)?;
    info!("Creating async database connection pool for {}", db_url);
    Ok(NostrPostgres::new(db_url).await?)
}

#[derive(Debug, Clone, Parser)]
pub struct RelayConfig {
    #[arg(default_value_t = String::from("postgres://postgres:password@localhost:5432"), long, env = "DB_CONNECTION_STRING")]
    pub db_connection_string: String,
    #[arg(default_value_t = String::from("localhost:8080"), long, env = "LISTEN_ADDRESS")]
    pub listen_address: String,
}
