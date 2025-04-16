use anyhow::Result;
use nostr_postgresdb::*;
use nostr_relay_builder::{
    LocalRelay, RelayBuilder,
    builder::{RelayBuilderNip42, RelayBuilderNip42Mode},
};

const DB_URL: &str = "postgres://postgres:password@localhost:5432";

pub async fn init() -> Result<LocalRelay> {
    Ok(LocalRelay::new(builder().await?).await?)
}

async fn builder() -> Result<RelayBuilder> {
    let dba = database().await?;
    Ok(RelayBuilder::default().nip42(auth_mode()).database(dba))
}

fn auth_mode() -> RelayBuilderNip42 {
    RelayBuilderNip42 {
        // read and write requires client auth
        mode: RelayBuilderNip42Mode::Both,
    }
}

async fn database() -> Result<NostrPostgres> {
    nostr_postgresdb::run_migrations(DB_URL)?;
    Ok(nostr_postgresdb::postgres_connection_pool(DB_URL)
        .await?
        .into())
}
