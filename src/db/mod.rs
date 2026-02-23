use deadpool_postgres::Pool;

pub struct PostgresStore {
    pub pool: Pool,
}

impl PostgresStore {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Creates the tables, if they don't exist yet
    pub async fn init(&self) -> Result<(), anyhow::Error> {
        // File Store
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
