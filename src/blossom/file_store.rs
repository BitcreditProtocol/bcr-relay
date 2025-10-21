use std::str::FromStr;

use crate::db::PostgresStore;
use async_trait::async_trait;
use diesel::prelude::*;
use diesel::sql_types::{Bytea, Integer, Text};
use diesel_async::RunQueryDsl;
use nostr::hashes::sha256::Hash as Sha256Hash;

use super::File;

#[derive(QueryableByName, Debug)]
struct DbFile {
    #[diesel(sql_type = Text)]
    hash: String,
    #[diesel(sql_type = Bytea)]
    data: Vec<u8>,
    #[diesel(sql_type = Integer)]
    size: i32,
}

#[async_trait]
pub trait FileStoreApi: Send + Sync {
    async fn get(&self, hash: &Sha256Hash) -> Result<Option<File>, anyhow::Error>;
    async fn insert(&self, file: File) -> Result<(), anyhow::Error>;
}

#[async_trait]
impl FileStoreApi for PostgresStore {
    async fn get(&self, hash: &Sha256Hash) -> Result<Option<File>, anyhow::Error> {
        let hash_str = hash.to_string();
        let mut conn = self.get_connection().await?;
        
        let result: Option<DbFile> = diesel::sql_query(
            "SELECT hash, data, size FROM files WHERE hash = $1"
        )
        .bind::<Text, _>(&hash_str)
        .get_result(&mut conn)
        .await
        .optional()?;

        match result {
            Some(db) => {
                let hash = Sha256Hash::from_str(&db.hash)?;
                Ok(Some(File { 
                    hash, 
                    bytes: db.data, 
                    size: db.size 
                }))
            }
            None => Ok(None),
        }
    }

    async fn insert(&self, file: File) -> Result<(), anyhow::Error> {
        let hash_str = file.hash.to_string();
        let mut conn = self.get_connection().await?;

        diesel::sql_query(
            "INSERT INTO files (hash, data, size) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"
        )
        .bind::<Text, _>(&hash_str)
        .bind::<Bytea, _>(&file.bytes)
        .bind::<Integer, _>(&file.size)
        .execute(&mut conn)
        .await?;

        Ok(())
    }
}
