use anyhow::Result;
use nostr_ndb::NdbDatabase;
use nostr_relay_builder::{
    LocalRelay, RelayBuilder,
    builder::{RelayBuilderNip42, RelayBuilderNip42Mode},
};

pub async fn init() -> Result<LocalRelay> {
    Ok(LocalRelay::new(builder()?).await?)
}

fn builder() -> Result<RelayBuilder> {
    Ok(RelayBuilder::default()
        .nip42(auth_mode())
        .database(database()?))
}

fn auth_mode() -> RelayBuilderNip42 {
    RelayBuilderNip42 {
        // read and write requires client auth
        mode: RelayBuilderNip42Mode::Both,
    }
}

fn database() -> Result<NdbDatabase> {
    Ok(NdbDatabase::open("./data")?)
}
