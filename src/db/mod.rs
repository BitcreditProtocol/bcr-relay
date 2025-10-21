use diesel_async::{AsyncPgConnection, pooled_connection::AsyncDieselConnectionManager, RunQueryDsl};
use deadpool::managed::Pool;

pub struct PostgresStore {
    pool: Pool<AsyncDieselConnectionManager<AsyncPgConnection>>,
}

impl PostgresStore {
    pub fn new(pool: Pool<AsyncDieselConnectionManager<AsyncPgConnection>>) -> Self {
        Self { pool }
    }

    pub async fn get_connection(
        &self,
    ) -> Result<
        deadpool::managed::Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        anyhow::Error,
    > {
        Ok(self.pool.get().await?)
    }

    /// Creates the tables, if they don't exist yet
    pub async fn init(&self) -> Result<(), anyhow::Error> {
        let mut conn = self.get_connection().await?;
        
        // File Store
        diesel::sql_query(
            r#"
            CREATE TABLE IF NOT EXISTS files (
                hash CHAR(64) PRIMARY KEY,
                data BYTEA NOT NULL,
                size INTEGER NOT NULL
            )
        "#,
        )
        .execute(&mut conn)
        .await?;

        // Notification Store
        diesel::sql_query(
            r#"
            CREATE TABLE IF NOT EXISTS notif_challenges (
                npub TEXT PRIMARY KEY,
                challenge TEXT NOT NULL,
                created_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'UTC')
            )
        "#,
        )
        .execute(&mut conn)
        .await?;

        diesel::sql_query(
            r#"
            CREATE TABLE IF NOT EXISTS notif_email_verification (
                npub TEXT PRIMARY KEY,
                email TEXT NOT NULL,
                confirmed BOOLEAN DEFAULT FALSE,
                token TEXT,
                sent_at TIMESTAMPTZ DEFAULT (NOW() AT TIME ZONE 'UTC')
            )
        "#,
        )
        .execute(&mut conn)
        .await?;

        diesel::sql_query(
            r#"
            CREATE TABLE IF NOT EXISTS notif_email_preferences (
                npub TEXT PRIMARY KEY,
                enabled BOOLEAN DEFAULT FALSE,
                token TEXT NOT NULL,
                email TEXT NOT NULL,
                email_confirmed BOOLEAN DEFAULT FALSE,
                ebill_url TEXT NOT NULL,
                flags BIGINT NOT NULL
            )
        "#,
        )
        .execute(&mut conn)
        .await?;

        Ok(())
    }
}
