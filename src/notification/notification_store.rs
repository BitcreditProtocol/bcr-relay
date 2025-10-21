use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Bool, Text, Timestamptz};
use diesel_async::{AsyncConnection, RunQueryDsl};
use diesel_async::scoped_futures::ScopedFutureExt;

use crate::{
    db::PostgresStore,
    notification::{Challenge, EmailConfirmation, EmailPreferences, PreferencesFlags},
};

#[derive(QueryableByName, Debug)]
struct DbChallenge {
    #[diesel(sql_type = Text)]
    npub: String,
    #[diesel(sql_type = Text)]
    challenge: String,
    #[diesel(sql_type = Timestamptz)]
    created_at: DateTime<Utc>,
}

#[derive(QueryableByName, Debug)]
struct DbEmailConfirmation {
    #[diesel(sql_type = Text)]
    npub: String,
    #[diesel(sql_type = Text)]
    email: String,
    #[diesel(sql_type = Bool)]
    confirmed: bool,
    #[diesel(sql_type = Timestamptz)]
    sent_at: DateTime<Utc>,
}

#[derive(QueryableByName, Debug)]
struct DbEmailPreferences {
    #[diesel(sql_type = Text)]
    npub: String,
    #[diesel(sql_type = Bool)]
    enabled: bool,
    #[diesel(sql_type = Text)]
    token: String,
    #[diesel(sql_type = Text)]
    email: String,
    #[diesel(sql_type = Bool)]
    email_confirmed: bool,
    #[diesel(sql_type = Text)]
    ebill_url: String,
    #[diesel(sql_type = BigInt)]
    flags: i64,
}

#[async_trait]
pub trait NotificationStoreApi: Send + Sync {
    async fn insert_challenge_for_npub(
        &self,
        npub: String,
        challenge: String,
    ) -> Result<(), anyhow::Error>;

    async fn get_challenge_for_npub(&self, npub: &str) -> Result<Option<Challenge>, anyhow::Error>;
    async fn remove_challenge_for_npub(&self, npub: &str) -> Result<(), anyhow::Error>;
    async fn insert_confirmation_email_sent_and_preferences_for_npub(
        &self,
        npub: &str,
        email: &str,
        confirmation_token: &str,
        preferences_token: &str,
        ebill_url: &str,
        flags: PreferencesFlags,
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
    async fn get_email_preferences_for_token(
        &self,
        token: &str,
    ) -> Result<Option<EmailPreferences>, anyhow::Error>;
    async fn update_email_preferences_for_token(
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
        let mut conn = self.get_connection().await?;
        
        diesel::sql_query(
            "INSERT INTO notif_challenges (npub, challenge) VALUES ($1, $2) ON CONFLICT (npub) DO UPDATE SET challenge = $2, created_at = (NOW() AT TIME ZONE 'UTC')"
        )
        .bind::<Text, _>(&npub)
        .bind::<Text, _>(&challenge)
        .execute(&mut conn)
        .await?;
        
        Ok(())
    }

    async fn get_challenge_for_npub(&self, npub: &str) -> Result<Option<Challenge>, anyhow::Error> {
        let mut conn = self.get_connection().await?;
        
        let result: Option<DbChallenge> = diesel::sql_query(
            "SELECT npub, challenge, created_at FROM notif_challenges WHERE npub = $1"
        )
        .bind::<Text, _>(npub)
        .get_result(&mut conn)
        .await
        .optional()?;

        match result {
            Some(db) => {
                Ok(Some(Challenge {
                    npub: db.npub,
                    challenge: db.challenge,
                    created_at: db.created_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn remove_challenge_for_npub(&self, npub: &str) -> Result<(), anyhow::Error> {
        let mut conn = self.get_connection().await?;
        
        diesel::sql_query("DELETE FROM notif_challenges WHERE npub = $1")
            .bind::<Text, _>(npub)
            .execute(&mut conn)
            .await?;
        
        Ok(())
    }

    async fn insert_confirmation_email_sent_and_preferences_for_npub(
        &self,
        npub: &str,
        email: &str,
        confirmation_token: &str,
        preferences_token: &str,
        ebill_url: &str,
        flags: PreferencesFlags,
    ) -> Result<(), anyhow::Error> {
        let mut conn = self.get_connection().await?;
        let flags_i64 = flags.bits();
        
        conn.transaction::<_, anyhow::Error, _>(|conn| {
            async move {
                diesel::sql_query(
                    "INSERT INTO notif_email_verification (npub, email, token) VALUES ($1, $2, $3) ON CONFLICT (npub) DO UPDATE SET email = $2, token = $3, confirmed = false, sent_at = (NOW() AT TIME ZONE 'UTC')"
                )
                .bind::<Text, _>(npub)
                .bind::<Text, _>(email)
                .bind::<Text, _>(confirmation_token)
                .execute(conn)
                .await?;

                diesel::sql_query(
                    "INSERT INTO notif_email_preferences (npub, email, token, ebill_url, flags) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (npub) DO UPDATE SET email = $2, token = $3, ebill_url = $4, flags = $5, enabled = false, email_confirmed = false"
                )
                .bind::<Text, _>(npub)
                .bind::<Text, _>(email)
                .bind::<Text, _>(preferences_token)
                .bind::<Text, _>(ebill_url)
                .bind::<BigInt, _>(flags_i64)
                .execute(conn)
                .await?;
                
                Ok(())
            }
            .scope_boxed()
        })
        .await?;
        
        Ok(())
    }

    async fn get_confirmation_email_state_for_token(
        &self,
        token: &str,
    ) -> Result<Option<EmailConfirmation>, anyhow::Error> {
        let mut conn = self.get_connection().await?;
        
        let result: Option<DbEmailConfirmation> = diesel::sql_query(
            "SELECT npub, email, confirmed, sent_at FROM notif_email_verification WHERE token = $1"
        )
        .bind::<Text, _>(token)
        .get_result(&mut conn)
        .await
        .optional()?;

        match result {
            Some(db) => {
                Ok(Some(EmailConfirmation {
                    npub: db.npub,
                    email: db.email,
                    confirmed: db.confirmed,
                    sent_at: db.sent_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn set_confirmation_email_confirmed_for_npub(
        &self,
        npub: &str,
    ) -> Result<(), anyhow::Error> {
        let mut conn = self.get_connection().await?;
        
        conn.transaction::<_, anyhow::Error, _>(|conn| {
            async move {
                diesel::sql_query("DELETE FROM notif_email_verification WHERE npub = $1")
                    .bind::<Text, _>(npub)
                    .execute(conn)
                    .await?;

                diesel::sql_query(
                    "UPDATE notif_email_preferences SET email_confirmed = true, enabled = true WHERE npub = $1"
                )
                .bind::<Text, _>(npub)
                .execute(conn)
                .await?;
                
                Ok(())
            }
            .scope_boxed()
        })
        .await?;
        
        Ok(())
    }

    async fn get_email_preferences_for_npub(
        &self,
        npub: &str,
    ) -> Result<Option<EmailPreferences>, anyhow::Error> {
        let mut conn = self.get_connection().await?;
        
        let result: Option<DbEmailPreferences> = diesel::sql_query(
            "SELECT npub, enabled, token, email, email_confirmed, ebill_url, flags FROM notif_email_preferences WHERE npub = $1"
        )
        .bind::<Text, _>(npub)
        .get_result(&mut conn)
        .await
        .optional()?;

        match result {
            Some(db) => {
                Ok(Some(EmailPreferences {
                    npub: db.npub,
                    enabled: db.enabled,
                    token: db.token,
                    email: db.email,
                    email_confirmed: db.email_confirmed,
                    ebill_url: url::Url::parse(&db.ebill_url)?,
                    flags: PreferencesFlags::from_bits_truncate(db.flags),
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_email_preferences_for_token(
        &self,
        token: &str,
    ) -> Result<Option<EmailPreferences>, anyhow::Error> {
        let mut conn = self.get_connection().await?;
        
        let result: Option<DbEmailPreferences> = diesel::sql_query(
            "SELECT npub, enabled, token, email, email_confirmed, ebill_url, flags FROM notif_email_preferences WHERE token = $1"
        )
        .bind::<Text, _>(token)
        .get_result(&mut conn)
        .await
        .optional()?;

        match result {
            Some(db) => {
                Ok(Some(EmailPreferences {
                    npub: db.npub,
                    enabled: db.enabled,
                    token: db.token,
                    email: db.email,
                    email_confirmed: db.email_confirmed,
                    ebill_url: url::Url::parse(&db.ebill_url)?,
                    flags: PreferencesFlags::from_bits_truncate(db.flags),
                }))
            }
            None => Ok(None),
        }
    }

    async fn update_email_preferences_for_token(
        &self,
        token: &str,
        enabled: bool,
        flags: PreferencesFlags,
    ) -> Result<(), anyhow::Error> {
        let mut conn = self.get_connection().await?;
        let flags_i64 = flags.bits();
        
        diesel::sql_query(
            "UPDATE notif_email_preferences SET enabled = $2, flags = $3 WHERE token = $1"
        )
        .bind::<Text, _>(token)
        .bind::<Bool, _>(enabled)
        .bind::<BigInt, _>(flags_i64)
        .execute(&mut conn)
        .await?;
        
        Ok(())
    }
}
