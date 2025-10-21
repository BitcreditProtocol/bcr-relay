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

        // Notification Store
        let qry = r#"
            CREATE TABLE IF NOT EXISTS notif_challenges (
                npub TEXT PRIMARY KEY,
                challenge TEXT NOT NULL,
                created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'UTC')
            )
        "#;
        self.pool.get().await?.execute(qry, &[]).await?;

        let qry = r#"
            CREATE TABLE IF NOT EXISTS notif_email_verification (
                npub TEXT PRIMARY KEY,
                email TEXT NOT NULL,
                confirmed BOOLEAN DEFAULT FALSE,
                token TEXT,
                sent_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'UTC')
            )
        "#;
        self.pool.get().await?.execute(qry, &[]).await?;

        let qry = r#"
            CREATE TABLE IF NOT EXISTS notif_email_preferences (
                npub TEXT PRIMARY KEY,
                enabled BOOLEAN DEFAULT FALSE,
                token TEXT NOT NULL,
                email TEXT NOT NULL,
                email_confirmed BOOLEAN DEFAULT FALSE,
                ebill_url TEXT NOT NULL,
                flags BIGINT NOT NULL
            )
        "#;
        self.pool.get().await?.execute(qry, &[]).await?;
        Ok(())
    }
}
