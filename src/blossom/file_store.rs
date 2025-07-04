use std::str::FromStr;

use async_trait::async_trait;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use nostr::hashes::sha256::Hash as Sha256Hash;
use tokio_postgres::{NoTls, Row};

use super::File;

#[async_trait]
pub trait FileStoreApi: Send + Sync {
    async fn get(&self, hash: &Sha256Hash) -> Result<Option<File>, anyhow::Error>;
    async fn insert(&self, file: File) -> Result<(), anyhow::Error>;
}

pub struct PostgresFileStore {
    pool: Pool,
}

impl PostgresFileStore {
    pub async fn new(conn_str: &str) -> Result<Self, anyhow::Error> {
        let cfg: tokio_postgres::Config = conn_str.parse()?;
        let mgr_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };
        let pool = Pool::builder(Manager::from_config(cfg, NoTls, mgr_config))
            .max_size(16)
            .build()?;

        Ok(Self { pool })
    }

    /// Creates the table, if it doesn't exist yet
    pub async fn init(&self) -> Result<(), anyhow::Error> {
        let qry = r#"
            CREATE TABLE IF NOT EXISTS files (
                hash CHAR(64) PRIMARY KEY,
                data BYTEA NOT NULL,
                size INTEGER NOT NULL
            )
        "#;

        self.pool.get().await?.execute(qry, &[]).await?;
        Ok(())
    }
}

#[async_trait]
impl FileStoreApi for PostgresFileStore {
    async fn get(&self, hash: &Sha256Hash) -> Result<Option<File>, anyhow::Error> {
        let row = self
            .pool
            .get()
            .await?
            .query_opt(
                "SELECT hash, data, size FROM files WHERE hash = $1",
                &[&hash.to_string()],
            )
            .await?;
        let db_file = row.map(|r| row_to_db_file(&r));

        match db_file {
            Some(f) => File::try_from(f).map(Some),
            None => return Ok(None),
        }
    }

    async fn insert(&self, file: File) -> Result<(), anyhow::Error> {
        let db_file: DbFile = file.into();

        self.pool
            .get()
            .await?
            .execute(
                "INSERT INTO files (hash, data, size) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
                &[&db_file.hash, &db_file.bytes, &db_file.size],
            )
            .await?;

        Ok(())
    }
}

fn row_to_db_file(row: &Row) -> DbFile {
    let hash: String = row.get(0);
    let data: Vec<u8> = row.get(1);
    let size: i32 = row.get(2);

    DbFile {
        hash,
        bytes: data,
        size,
    }
}

#[derive(Debug)]
pub struct DbFile {
    pub hash: String,
    pub bytes: Vec<u8>,
    pub size: i32,
}

impl TryFrom<DbFile> for File {
    type Error = anyhow::Error;

    fn try_from(value: DbFile) -> Result<Self, Self::Error> {
        let hash = Sha256Hash::from_str(&value.hash)?;
        Ok(Self {
            hash,
            bytes: value.bytes,
            size: value.size,
        })
    }
}

impl From<File> for DbFile {
    fn from(value: File) -> Self {
        Self {
            hash: value.hash.to_string(),
            bytes: value.bytes,
            size: value.size,
        }
    }
}
