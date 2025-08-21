use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio_postgres::Row;

use crate::{
    db::PostgresStore,
    notification::{Challenge, EmailConfirmation, EmailPreferences, PreferencesFlags},
};

#[async_trait]
pub trait NotificationStoreApi: Send + Sync {
    async fn insert_challenge_for_npub(
        &self,
        npub: String,
        challenge: String,
    ) -> Result<(), anyhow::Error>;

    async fn get_challenge_for_npub(&self, npub: &str) -> Result<Option<Challenge>, anyhow::Error>;
    async fn remove_challenge_for_npub(&self, npub: &str) -> Result<(), anyhow::Error>;
    async fn insert_confirmation_email_sent_for_npub(
        &self,
        npub: &str,
        email: &str,
        token: &str,
    ) -> Result<(), anyhow::Error>;
    async fn get_confirmation_email_state_for_token(
        &self,
        token: &str,
    ) -> Result<Option<EmailConfirmation>, anyhow::Error>;
    async fn set_confirmation_email_confirmed_for_npub(
        &self,
        npub: &str,
    ) -> Result<(), anyhow::Error>;
    async fn get_email_preferences_for_npub(
        &self,
        npub: &str,
    ) -> Result<Option<EmailPreferences>, anyhow::Error>;
    async fn insert_email_preferences_for_npub(
        &self,
        npub: &str,
        email: &str,
        token: &str,
        ebill_url: &str,
        flags: PreferencesFlags,
    ) -> Result<(), anyhow::Error>;
    #[allow(unused)]
    async fn update_email_preferences_for_npub(
        &self,
        npub: &str,
        enabled: bool,
        flags: PreferencesFlags,
    ) -> Result<(), anyhow::Error>;
}

#[async_trait]
impl NotificationStoreApi for PostgresStore {
    async fn insert_challenge_for_npub(
        &self,
        npub: String,
        challenge: String,
    ) -> Result<(), anyhow::Error> {
        self.pool
            .get()
            .await?
            .execute(
                "INSERT INTO notif_challenges (npub, challenge) VALUES ($1, $2) ON CONFLICT (npub) DO UPDATE SET challenge = $2, created_at = (NOW() AT TIME ZONE 'UTC')",
                &[&npub, &challenge],
            )
            .await?;
        Ok(())
    }

    async fn get_challenge_for_npub(&self, npub: &str) -> Result<Option<Challenge>, anyhow::Error> {
        let row = self
            .pool
            .get()
            .await?
            .query_opt(
                "SELECT npub, challenge, created_at FROM notif_challenges WHERE npub = $1",
                &[&npub.to_string()],
            )
            .await?;
        let db_challenge = row.map(|r| row_to_db_challenge(&r));

        match db_challenge {
            Some(c) => Challenge::try_from(c).map(Some),
            None => return Ok(None),
        }
    }

    async fn remove_challenge_for_npub(&self, npub: &str) -> Result<(), anyhow::Error> {
        self.pool
            .get()
            .await?
            .execute("DELETE FROM notif_challenges WHERE npub = $1", &[&npub])
            .await?;
        Ok(())
    }

    async fn insert_confirmation_email_sent_for_npub(
        &self,
        npub: &str,
        email: &str,
        token: &str,
    ) -> Result<(), anyhow::Error> {
        self.pool
            .get()
            .await?
            .execute(
                "INSERT INTO notif_email_verification (npub, email, token) VALUES ($1, $2, $3) ON CONFLICT (npub) DO UPDATE SET email = $2, token = $3, confirmed = false, sent_at = (NOW() AT TIME ZONE 'UTC')",
                &[&npub, &email, &token],
            )
            .await?;
        Ok(())
    }

    async fn get_confirmation_email_state_for_token(
        &self,
        token: &str,
    ) -> Result<Option<EmailConfirmation>, anyhow::Error> {
        let row = self
            .pool
            .get()
            .await?
            .query_opt(
                "SELECT npub, email, confirmed, sent_at FROM notif_email_verification WHERE token = $1",
                &[&token.to_string()],
            )
            .await?;
        let db_email_confirmation = row.map(|r| row_to_db_email_confirmation(&r));

        match db_email_confirmation {
            Some(c) => EmailConfirmation::try_from(c).map(Some),
            None => return Ok(None),
        }
    }

    async fn set_confirmation_email_confirmed_for_npub(
        &self,
        npub: &str,
    ) -> Result<(), anyhow::Error> {
        self.pool
            .get()
            .await?
            .execute(
                "DELETE FROM notif_email_verification WHERE npub = $1",
                &[&npub],
            )
            .await?;

        self.pool
            .get()
            .await?
            .execute(
                "UPDATE notif_email_preferences SET email_confirmed = true, enabled = true WHERE npub = $1",
                &[&npub],
            )
            .await?;
        Ok(())
    }

    async fn get_email_preferences_for_npub(
        &self,
        npub: &str,
    ) -> Result<Option<EmailPreferences>, anyhow::Error> {
        let row = self
            .pool
            .get()
            .await?
            .query_opt(
                "SELECT npub, enabled, email, email_confirmed, ebill_url, flags FROM notif_email_preferences WHERE npub = $1",
                &[&npub.to_string()],
            )
            .await?;
        let db_email_preferences = row.map(|r| row_to_db_email_preferences(&r));

        match db_email_preferences {
            Some(c) => EmailPreferences::try_from(c).map(Some),
            None => return Ok(None),
        }
    }

    async fn insert_email_preferences_for_npub(
        &self,
        npub: &str,
        email: &str,
        token: &str,
        ebill_url: &str,
        flags: PreferencesFlags,
    ) -> Result<(), anyhow::Error> {
        self.pool
            .get()
            .await?
            .execute(
                "INSERT INTO notif_email_preferences (npub, email, token, ebill_url, flags) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (npub) DO UPDATE SET email = $2, token = $3, ebill_url = $4, flags = $5, enabled = false, email_confirmed = false",
                &[&npub, &email, &token, &ebill_url, &{ flags.bits() }],
            )
            .await?;
        Ok(())
    }

    async fn update_email_preferences_for_npub(
        &self,
        npub: &str,
        enabled: bool,
        flags: PreferencesFlags,
    ) -> Result<(), anyhow::Error> {
        self.pool
            .get()
            .await?
            .execute(
                "UPDATE notif_email_preferences SET enabled = $2, flags = $3 WHERE npub = $1",
                &[&npub, &enabled, &{ flags.bits() }],
            )
            .await?;
        Ok(())
    }
}

fn row_to_db_challenge(row: &Row) -> DbChallenge {
    let npub: String = row.get(0);
    let challenge: String = row.get(1);
    let created_at: DateTime<Utc> = row.get(2);

    DbChallenge {
        npub,
        challenge,
        created_at,
    }
}

#[derive(Debug)]
pub struct DbChallenge {
    pub npub: String,
    pub challenge: String,
    pub created_at: DateTime<Utc>,
}

impl TryFrom<DbChallenge> for Challenge {
    type Error = anyhow::Error;

    fn try_from(value: DbChallenge) -> Result<Self, Self::Error> {
        Ok(Self {
            npub: value.npub,
            challenge: value.challenge,
            created_at: value.created_at,
        })
    }
}

fn row_to_db_email_confirmation(row: &Row) -> DbEmailConfirmation {
    let npub: String = row.get(0);
    let email: String = row.get(1);
    let confirmed: bool = row.get(2);
    let sent_at: DateTime<Utc> = row.get(3);

    DbEmailConfirmation {
        npub,
        email,
        confirmed,
        sent_at,
    }
}

#[derive(Debug)]
pub struct DbEmailConfirmation {
    pub npub: String,
    pub email: String,
    pub confirmed: bool,
    pub sent_at: DateTime<Utc>,
}

impl TryFrom<DbEmailConfirmation> for EmailConfirmation {
    type Error = anyhow::Error;

    fn try_from(value: DbEmailConfirmation) -> Result<Self, Self::Error> {
        Ok(Self {
            npub: value.npub,
            email: value.email,
            confirmed: value.confirmed,
            sent_at: value.sent_at,
        })
    }
}

fn row_to_db_email_preferences(row: &Row) -> DbEmailPreferences {
    let npub: String = row.get(0);
    let enabled: bool = row.get(1);
    let email: String = row.get(2);
    let email_confirmed: bool = row.get(3);
    let ebill_url: String = row.get(4);
    let flags: i64 = row.get(5);

    DbEmailPreferences {
        npub,
        enabled,
        email,
        email_confirmed,
        ebill_url,
        flags,
    }
}

#[derive(Debug)]
pub struct DbEmailPreferences {
    pub npub: String,
    pub enabled: bool,
    pub email: String,
    pub email_confirmed: bool,
    pub ebill_url: String,
    pub flags: i64,
}

impl TryFrom<DbEmailPreferences> for EmailPreferences {
    type Error = anyhow::Error;

    fn try_from(value: DbEmailPreferences) -> Result<Self, Self::Error> {
        Ok(Self {
            npub: value.npub,
            enabled: value.enabled,
            email: value.email,
            email_confirmed: value.email_confirmed,
            ebill_url: url::Url::parse(&value.ebill_url)?,
            flags: PreferencesFlags::from_bits_truncate(value.flags),
        })
    }
}
